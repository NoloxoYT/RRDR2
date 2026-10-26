using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using RDR2.Math;
using RDR2.Native;

namespace RRDR.Host
{
    /// <summary>
    /// Loads a Rust cdylib and forwards the ScriptHook tick to it.
    ///
    /// The native -> managed direction is a function-pointer table: Rust calls
    /// OnCallNative(hash, args), which builds ScriptHook InputArguments and calls
    /// RDR2.Native.Function.Call. That keeps every native call on the script
    /// thread and avoids reimplementing the ScriptHook native stack in Rust.
    /// </summary>
    internal sealed class RustBridge : IDisposable
    {
        private const uint BridgeAbiVersion = 3;

        private const int ArgI32 = 0;
        private const int ArgU32 = 1;
        private const int ArgF32 = 2;
        private const int ArgBool = 3;
        private const int ArgU64 = 4;
        private const int ArgString = 5;
        private const int ArgPointer = 6;

        private const uint ReturnU64 = 0;
        private const uint ReturnVec3 = 1;
        private const uint ReturnString = 2;

        private const int RtldNow = 2;

        private static readonly bool IsWindows =
            Environment.OSVersion.Platform == PlatformID.Win32NT;

        private readonly HostLog _log;
        private readonly CallNativeDelegate _callNative;
        private readonly LogDelegate _logCallback;
        private readonly AbiVersionDelegate _abiVersion;
        private readonly InitDelegate _init;
        private readonly InitV3Delegate _initV3;
        private readonly PoolHandlesDelegate _poolHandles;
        private readonly TickDelegate _tick;
        private readonly ShutdownDelegate _shutdown;
        private readonly ReloadDelegate _reload;

        private IntPtr _module;
        private bool _initialized;
        private bool _disposed;

        public RustBridge(string baseDirectory, HostConfig config, HostLog log)
        {
            _log = log;
            _callNative = OnCallNative;
            _logCallback = OnLog;
            _poolHandles = OnPoolHandles;

            string corePath = ResolveCorePath(baseDirectory, config.CoreDll);
            _module = LoadNativeLibrary(corePath);

            try
            {
                _abiVersion = GetDelegate<AbiVersionDelegate>("rd_abi_version");
                _tick = GetDelegate<TickDelegate>("rd_tick");
                _shutdown = GetDelegate<ShutdownDelegate>("rd_shutdown");
                _reload = GetDelegate<ReloadDelegate>("rd_reload_config");

                uint abi = _abiVersion();
                if (abi != 2 && abi != BridgeAbiVersion)
                {
                    throw new InvalidOperationException(string.Format(
                        "Rust core ABI v{0} is not supported (host expects v2 or v{1}). Rebuild the core.",
                        abi,
                        BridgeAbiVersion));
                }

                string configPath = HostConfig.ResolvePath(baseDirectory, config.CoreConfig);

                if (abi >= 3)
                {
                    _initV3 = GetDelegate<InitV3Delegate>("rd_init_v3");
                    IntPtr hostPointer = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(RdHostV3)));
                    try
                    {
                        RdHostV3 host = new RdHostV3
                        {
                            User = IntPtr.Zero,
                            CallNative = Marshal.GetFunctionPointerForDelegate(_callNative),
                            Log = Marshal.GetFunctionPointerForDelegate(_logCallback),
                            PoolHandles = Marshal.GetFunctionPointerForDelegate(_poolHandles)
                        };
                        Marshal.StructureToPtr(host, hostPointer, false);
                        CallInitV3(hostPointer, configPath, _initV3);
                    }
                    finally
                    {
                        Marshal.FreeHGlobal(hostPointer);
                    }
                }
                else
                {
                    _init = GetDelegate<InitDelegate>("rd_init");
                    IntPtr hostPointer = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(RdHost)));
                    try
                    {
                        RdHost host = new RdHost
                        {
                            User = IntPtr.Zero,
                            CallNative = Marshal.GetFunctionPointerForDelegate(_callNative),
                            Log = Marshal.GetFunctionPointerForDelegate(_logCallback)
                        };
                        Marshal.StructureToPtr(host, hostPointer, false);
                        CallInit(hostPointer, configPath, _init);
                    }
                    finally
                    {
                        Marshal.FreeHGlobal(hostPointer);
                    }
                }

                _initialized = true;
                _log.Info("Rust core loaded: " + corePath);
            }
            catch
            {
                Unload();
                throw;
            }
        }

        private void CallInit(IntPtr hostPointer, string configPath, InitDelegate init)
        {
            IntPtr configPointer = AllocUtf8(configPath);
            try
            {
                int result = init(hostPointer, configPointer);
                if (result != 0)
                {
                    throw new InvalidOperationException(string.Format(
                        "rd_init failed with code {0}. See the host log for the Rust error.",
                        result));
                }
            }
            finally
            {
                if (configPointer != IntPtr.Zero)
                {
                    Marshal.FreeHGlobal(configPointer);
                }
            }
        }

        private void CallInitV3(IntPtr hostPointer, string configPath, InitV3Delegate init)
        {
            IntPtr configPointer = AllocUtf8(configPath);
            try
            {
                int result = init(hostPointer, configPointer);
                if (result != 0)
                {
                    throw new InvalidOperationException(string.Format(
                        "rd_init_v3 failed with code {0}. See the host log for the Rust error.",
                        result));
                }
            }
            finally
            {
                if (configPointer != IntPtr.Zero)
                {
                    Marshal.FreeHGlobal(configPointer);
                }
            }
        }

        public bool IsInitialized
        {
            get { return _initialized; }
        }

        public void Tick(float deltaSeconds)
        {
            if (!_initialized)
            {
                return;
            }

            _tick(deltaSeconds);
        }

        public bool Reload(string configPath)
        {
            if (!_initialized)
            {
                return false;
            }

            IntPtr configPointer = AllocUtf8(configPath);
            try
            {
                int result = _reload(configPointer);
                if (result != 0)
                {
                    _log.Error(string.Format("rd_reload_config failed with code {0}", result));
                    return false;
                }

                return true;
            }
            finally
            {
                if (configPointer != IntPtr.Zero)
                {
                    Marshal.FreeHGlobal(configPointer);
                }
            }
        }

        public void Dispose()
        {
            if (_disposed)
            {
                return;
            }

            _disposed = true;

            if (_initialized)
            {
                try
                {
                    _shutdown();
                }
                catch (Exception exception)
                {
                    _log.Error("rd_shutdown failed: " + exception);
                }

                _initialized = false;
            }

            Unload();
        }

        private ulong OnCallNative(
            IntPtr user,
            ulong hash,
            IntPtr arguments,
            uint argumentCount,
            uint returnKind,
            IntPtr returnBuffer,
            uint returnCapacity)
        {
            try
            {
                InputArgument[] inputArguments;

                if (argumentCount == 0)
                {
                    inputArguments = new InputArgument[0];
                }
                else
                {
                    int count = checked((int)argumentCount);
                    int size = Marshal.SizeOf(typeof(RdArg));
                    inputArguments = new InputArgument[count];

                    for (int index = 0; index < count; index++)
                    {
                        IntPtr address = IntPtr.Add(arguments, checked(index * size));
                        RdArg raw = (RdArg)Marshal.PtrToStructure(address, typeof(RdArg));
                        inputArguments[index] = ToInputArgument(raw);
                    }
                }

                if (returnKind == ReturnVec3)
                {
                    Vector3 vector = Function.Call<Vector3>(hash, inputArguments);
                    if (returnBuffer != IntPtr.Zero)
                    {
                        // Write exactly 12 bytes (X, Y, Z). Do not use
                        // StructureToPtr: the explicit-layout Vector3 used by
                        // ScriptHook may be padded.
                        float[] components = { vector.X, vector.Y, vector.Z };
                        Marshal.Copy(components, 0, returnBuffer, 3);
                    }
                    return 0;
                }

                if (returnKind == ReturnString)
                {
                    string value = Function.Call<string>(hash, inputArguments) ?? string.Empty;
                    byte[] bytes = Encoding.UTF8.GetBytes(value);
                    int capacity = checked((int)returnCapacity);
                    int length = Math.Min(bytes.Length, capacity);

                    if (returnBuffer != IntPtr.Zero && length > 0)
                    {
                        Marshal.Copy(bytes, 0, returnBuffer, length);
                    }

                    return (ulong)length;
                }

                return Function.Call<ulong>(hash, inputArguments);
            }
            catch (Exception exception)
            {
                _log.Error(string.Format(
                    "native 0x{0:X16} failed: {1}",
                    hash,
                    exception));
                return 0;
            }
        }

        private uint OnPoolHandles(IntPtr user, uint kind, IntPtr output, uint capacity)
        {
            try
            {
                int length = checked((int)capacity);
                if (length <= 0)
                {
                    return 0;
                }

                int[] buffer = new int[length];
                int count;

#if MOCK_API
                var handler = MockNativeHost.PoolHandler;
                count = handler == null ? 0 : handler(kind, buffer);
#else
                switch (kind)
                {
                    case 0:
                        count = WorldGetAllPeds(buffer, length);
                        break;
                    case 1:
                        count = WorldGetAllVehicles(buffer, length);
                        break;
                    case 2:
                        count = WorldGetAllObjects(buffer, length);
                        break;
                    default:
                        return 0;
                }
#endif

                count = Math.Max(0, Math.Min(count, length));
                if (count > 0 && output != IntPtr.Zero)
                {
                    Marshal.Copy(buffer, 0, output, count);
                }

                return (uint)count;
            }
            catch (Exception exception)
            {
                _log.Error("pool query failed: " + exception);
                return 0;
            }
        }

        private void OnLog(IntPtr user, uint level, IntPtr message, uint length)
        {
            try
            {
                byte[] bytes = new byte[checked((int)length)];
                if (bytes.Length > 0)
                {
                    Marshal.Copy(message, bytes, 0, bytes.Length);
                }

                _log.Rust(level, Encoding.UTF8.GetString(bytes));
            }
            catch (Exception exception)
            {
                _log.Error("Rust log callback failed: " + exception);
            }
        }

        private static InputArgument ToInputArgument(RdArg raw)
        {
            switch (raw.Kind)
            {
                case ArgI32:
                    return new InputArgument(unchecked((int)(uint)raw.Bits));
                case ArgU32:
                    return new InputArgument((uint)raw.Bits);
                case ArgF32:
                    return new InputArgument(BitConverter.ToSingle(
                        BitConverter.GetBytes((uint)raw.Bits),
                        0));
                case ArgBool:
                    return new InputArgument(raw.Bits != 0);
                case ArgString:
                    return new InputArgument(ReadUtf8(raw.Bits));
                case ArgU64:
                case ArgPointer:
                    return new InputArgument(raw.Bits);
                default:
                    return new InputArgument(raw.Bits);
            }
        }

        private static string ReadUtf8(ulong bits)
        {
            IntPtr pointer = new IntPtr(unchecked((long)bits));
            if (pointer == IntPtr.Zero)
            {
                return string.Empty;
            }

            const int maximumLength = 1024 * 1024;
            int length = 0;
            while (length < maximumLength && Marshal.ReadByte(pointer, length) != 0)
            {
                length++;
            }

            byte[] bytes = new byte[length];
            if (length > 0)
            {
                Marshal.Copy(pointer, bytes, 0, length);
            }

            return Encoding.UTF8.GetString(bytes);
        }

        private static IntPtr AllocUtf8(string value)
        {
            if (string.IsNullOrEmpty(value))
            {
                return IntPtr.Zero;
            }

            byte[] bytes = Encoding.UTF8.GetBytes(value);
            IntPtr pointer = Marshal.AllocHGlobal(bytes.Length + 1);
            Marshal.Copy(bytes, 0, pointer, bytes.Length);
            Marshal.WriteByte(pointer, bytes.Length, 0);
            return pointer;
        }

        private static string ResolveCorePath(string baseDirectory, string configured)
        {
            string name = string.IsNullOrEmpty(configured)
                ? "RealisticDamagesCore.dll"
                : configured;

            List<string> candidates = new List<string>();

            if (Path.IsPathRooted(name))
            {
                candidates.Add(name);
            }
            else
            {
                candidates.Add(Path.Combine(baseDirectory, name));
            }

            string stem = Path.GetFileNameWithoutExtension(name);
            if (IsWindows)
            {
                candidates.Add(Path.Combine(baseDirectory, stem + ".dll"));
            }
            else
            {
                candidates.Add(Path.Combine(baseDirectory, "lib" + stem + ".so"));
                candidates.Add(Path.Combine(baseDirectory, stem + ".so"));
            }

            foreach (string candidate in candidates)
            {
                if (File.Exists(candidate))
                {
                    return Path.GetFullPath(candidate);
                }
            }

            throw new FileNotFoundException(
                "Rust core DLL not found. Looked for:" + Environment.NewLine +
                string.Join(Environment.NewLine, candidates.ToArray()));
        }

        private static IntPtr LoadNativeLibrary(string path)
        {
            if (IsWindows)
            {
                IntPtr module = LoadLibraryW(path);
                if (module == IntPtr.Zero)
                {
                    throw new DllNotFoundException(string.Format(
                        "LoadLibraryW('{0}') failed, Win32 error {1}",
                        path,
                        Marshal.GetLastWin32Error()));
                }

                return module;
            }
            else
            {
                IntPtr module = DlOpen(path, RtldNow);
                if (module == IntPtr.Zero)
                {
                    throw new DllNotFoundException(string.Format(
                        "dlopen('{0}') failed: {1}",
                        path,
                        ReadDlError()));
                }

                return module;
            }
        }

        /// <summary>
        /// True when the file exports the RRDR bridge entry point. Used to skip
        /// ordinary native DLLs that happen to sit in the mods folder.
        /// </summary>
        internal static bool HasBridgeAbi(string path)
        {
            IntPtr module = IntPtr.Zero;

            try
            {
                module = LoadNativeLibrary(path);

                if (IsWindows)
                {
                    return GetProcAddress(module, "rd_abi_version") != IntPtr.Zero;
                }

                return DlSym(module, "rd_abi_version") != IntPtr.Zero;
            }
            catch
            {
                return false;
            }
            finally
            {
                if (module != IntPtr.Zero)
                {
                    if (IsWindows)
                    {
                        FreeLibrary(module);
                    }
                    else
                    {
                        DlClose(module);
                    }
                }
            }
        }

        private static IntPtr GetSymbol(IntPtr module, string name)
        {
            if (IsWindows)
            {
                IntPtr symbol = GetProcAddress(module, name);
                if (symbol == IntPtr.Zero)
                {
                    throw new EntryPointNotFoundException(string.Format(
                        "GetProcAddress('{0}') failed, Win32 error {1}",
                        name,
                        Marshal.GetLastWin32Error()));
                }

                return symbol;
            }
            else
            {
                IntPtr symbol = DlSym(module, name);
                if (symbol == IntPtr.Zero)
                {
                    throw new EntryPointNotFoundException(string.Format(
                        "dlsym('{0}') failed: {1}",
                        name,
                        ReadDlError()));
                }

                return symbol;
            }
        }

        private T GetDelegate<T>(string name) where T : class
        {
            IntPtr symbol = GetSymbol(_module, name);
            return (T)(object)Marshal.GetDelegateForFunctionPointer(symbol, typeof(T));
        }

        private void Unload()
        {
            if (_module == IntPtr.Zero)
            {
                return;
            }

            if (IsWindows)
            {
                FreeLibrary(_module);
            }
            else
            {
                DlClose(_module);
            }

            _module = IntPtr.Zero;
        }

        private static string ReadDlError()
        {
            try
            {
                IntPtr error = DlError();
                return error == IntPtr.Zero ? "unknown error" : Marshal.PtrToStringAnsi(error);
            }
            catch
            {
                return "unknown error";
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct RdArg
        {
            public uint Kind;
            public uint Pad;
            public ulong Bits;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct RdHost
        {
            public IntPtr User;
            public IntPtr CallNative;
            public IntPtr Log;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct RdHostV3
        {
            public IntPtr User;
            public IntPtr CallNative;
            public IntPtr Log;
            public IntPtr PoolHandles;
        }

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate uint AbiVersionDelegate();

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int InitDelegate(IntPtr host, IntPtr configPath);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int InitV3Delegate(IntPtr host, IntPtr configPath);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate uint PoolHandlesDelegate(
            IntPtr user,
            uint kind,
            IntPtr output,
            uint capacity);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void TickDelegate(float deltaSeconds);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void ShutdownDelegate();

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate int ReloadDelegate(IntPtr configPath);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate ulong CallNativeDelegate(
            IntPtr user,
            ulong hash,
            IntPtr arguments,
            uint argumentCount,
            uint returnKind,
            IntPtr returnBuffer,
            uint returnCapacity);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
        private delegate void LogDelegate(IntPtr user, uint level, IntPtr message, uint length);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr LoadLibraryW(string fileName);

        [DllImport("kernel32.dll", CharSet = CharSet.Ansi, SetLastError = true)]
        private static extern IntPtr GetProcAddress(IntPtr module, string procedureName);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool FreeLibrary(IntPtr module);

        [DllImport("libdl.so.2", EntryPoint = "dlopen")]
        private static extern IntPtr DlOpen(string fileName, int flags);

        [DllImport("libdl.so.2", EntryPoint = "dlsym")]
        private static extern IntPtr DlSym(IntPtr module, string symbol);

        [DllImport("libdl.so.2", EntryPoint = "dlclose")]
        private static extern int DlClose(IntPtr module);

        [DllImport("libdl.so.2", EntryPoint = "dlerror")]
        private static extern IntPtr DlError();

#if !MOCK_API
        [DllImport("ScriptHookRDR2.dll", EntryPoint = "?worldGetAllPeds@@YAHPEAHH@Z", CallingConvention = CallingConvention.Cdecl)]
        private static extern int WorldGetAllPeds([In, Out] int[] buffer, int capacity);

        [DllImport("ScriptHookRDR2.dll", EntryPoint = "?worldGetAllVehicles@@YAHPEAHH@Z", CallingConvention = CallingConvention.Cdecl)]
        private static extern int WorldGetAllVehicles([In, Out] int[] buffer, int capacity);

        [DllImport("ScriptHookRDR2.dll", EntryPoint = "?worldGetAllObjects@@YAHPEAHH@Z", CallingConvention = CallingConvention.Cdecl)]
        private static extern int WorldGetAllObjects([In, Out] int[] buffer, int capacity);
#endif
    }
}
