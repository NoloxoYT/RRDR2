// Offline reference / mock implementation of the parts of ScriptHookRDRNetAPI
// used by RRDR.Host. The assembly name and version match the real API
// (ScriptHookRDRNetAPI 2.2.0.0), so a host compiled against this file binds to
// the real assembly at runtime when the mock is not copied next to it.
//
// This assembly is NOT a ScriptHook plugin and must never be shipped in the
// game folder in place of the real ScriptHookRDRNetAPI.dll.

using System;
using System.Runtime.InteropServices;
using RDR2.Math;

namespace RDR2
{
    /// <summary>
    /// Minimal stand-in for RDR2.Script. The real class is provided by
    /// ScriptHookRDRNetAPI.dll in game; the member signatures used by the host
    /// are identical.
    /// </summary>
    public abstract class Script
    {
        public event EventHandler Tick;
        public event EventHandler Aborted;

        public int Interval { get; set; }

        public string Name { get; internal set; }
        public string FileName { get; internal set; }

        protected Script()
        {
            Name = string.Empty;
            FileName = string.Empty;
        }

        // Test-only helpers. They do not exist on the real class, and the host
        // never references them; the simulator uses them to drive the script.
        public void RaiseTick()
        {
            Tick?.Invoke(this, EventArgs.Empty);
        }

        public void RaiseAborted()
        {
            Aborted?.Invoke(this, EventArgs.Empty);
        }
    }
}

namespace RDR2.Native
{
    public enum InputArgumentKind
    {
        Int32,
        UInt32,
        UInt64,
        Single,
        Boolean,
        String
    }

    /// <summary>
    /// Minimal stand-in for RDR2.Native.InputArgument.
    /// </summary>
    public sealed class InputArgument
    {
        internal ulong data;

        public InputArgument(ulong value)
        {
            data = value;
            Kind = InputArgumentKind.UInt64;
            Value = value;
        }

        public InputArgument(bool value)
        {
            data = value ? 1UL : 0UL;
            Kind = InputArgumentKind.Boolean;
            Value = value;
        }

        public InputArgument(int value)
        {
            data = (uint)value;
            Kind = InputArgumentKind.Int32;
            Value = value;
        }

        public InputArgument(uint value)
        {
            data = value;
            Kind = InputArgumentKind.UInt32;
            Value = value;
        }

        public InputArgument(float value)
        {
            data = BitConverter.ToUInt32(BitConverter.GetBytes(value), 0);
            Kind = InputArgumentKind.Single;
            Value = value;
        }

        public InputArgument(string value)
        {
            data = 0;
            Kind = InputArgumentKind.String;
            Value = value ?? string.Empty;
        }

        public InputArgumentKind Kind { get; }

        public object Value { get; }

        public ulong RawValue => data;

        public int AsInt32()
        {
            return Convert.ToInt32(Value);
        }

        public uint AsUInt32()
        {
            return Convert.ToUInt32(Value);
        }

        public ulong AsUInt64()
        {
            return data;
        }

        public float AsSingle()
        {
            return Convert.ToSingle(Value);
        }

        public bool AsBoolean()
        {
            return Convert.ToBoolean(Value);
        }

        public string AsString()
        {
            return Value as string ?? string.Empty;
        }

        public static implicit operator InputArgument(bool value) => new InputArgument(value);
        public static implicit operator InputArgument(byte value) => new InputArgument((ulong)value);
        public static implicit operator InputArgument(sbyte value) => new InputArgument((ulong)value);
        public static implicit operator InputArgument(short value) => new InputArgument((ulong)value);
        public static implicit operator InputArgument(ushort value) => new InputArgument((ulong)value);
        public static implicit operator InputArgument(int value) => new InputArgument(value);
        public static implicit operator InputArgument(uint value) => new InputArgument(value);
        public static implicit operator InputArgument(ulong value) => new InputArgument(value);
        public static implicit operator InputArgument(float value) => new InputArgument(value);
        public static implicit operator InputArgument(double value) => new InputArgument((float)value);
        public static implicit operator InputArgument(string value) => new InputArgument(value);
    }

    /// <summary>
    /// Native dispatcher used by the offline simulator. In game the real
    /// Function class calls ScriptHookRDR2.dll instead.
    /// </summary>
    public delegate ulong MockNativeHandler(ulong hash, InputArgument[] arguments);

    public delegate Vector3 MockVector3Handler(ulong hash, InputArgument[] arguments);

    public delegate string MockStringHandler(ulong hash, InputArgument[] arguments);

    public delegate int MockPoolHandler(uint kind, int[] buffer);

    public static class MockNativeHost
    {
        public static MockNativeHandler Handler { get; set; }

        public static MockVector3Handler Vector3Handler { get; set; }

        public static MockStringHandler StringHandler { get; set; }

        public static MockPoolHandler PoolHandler { get; set; }

        internal static ulong Dispatch(ulong hash, InputArgument[] arguments)
        {
            var handler = Handler;
            return handler == null ? 0UL : handler(hash, arguments ?? Array.Empty<InputArgument>());
        }

        internal static Vector3 DispatchVector3(ulong hash, InputArgument[] arguments)
        {
            var handler = Vector3Handler;
            return handler == null
                ? default(Vector3)
                : handler(hash, arguments ?? Array.Empty<InputArgument>());
        }

        internal static string DispatchString(ulong hash, InputArgument[] arguments)
        {
            var handler = StringHandler;
            return handler == null
                ? string.Empty
                : handler(hash, arguments ?? Array.Empty<InputArgument>());
        }
    }

    /// <summary>
    /// Minimal stand-in for RDR2.Native.Function. The host only uses the raw
    /// ulong overload; the other conversions make the mock pleasant to use from
    /// test code.
    /// </summary>
    public static class Function
    {
        public static T Call<T>(ulong hash, params InputArgument[] arguments)
        {
            if (typeof(T) == typeof(Vector3))
            {
                object vector = MockNativeHost.DispatchVector3(hash, arguments);
                return (T)vector;
            }

            if (typeof(T) == typeof(string))
            {
                object text = MockNativeHost.DispatchString(hash, arguments);
                return (T)text;
            }

            ulong raw = MockNativeHost.Dispatch(hash, arguments);
            object result;

            if (typeof(T) == typeof(ulong))
            {
                result = raw;
            }
            else if (typeof(T) == typeof(long))
            {
                result = unchecked((long)raw);
            }
            else if (typeof(T) == typeof(int))
            {
                result = unchecked((int)(uint)raw);
            }
            else if (typeof(T) == typeof(uint))
            {
                result = (uint)raw;
            }
            else if (typeof(T) == typeof(float))
            {
                result = BitConverter.ToSingle(BitConverter.GetBytes((uint)raw), 0);
            }
            else if (typeof(T) == typeof(double))
            {
                result = (double)BitConverter.ToSingle(BitConverter.GetBytes((uint)raw), 0);
            }
            else if (typeof(T) == typeof(bool))
            {
                result = raw != 0;
            }
            else
            {
                result = default(T);
            }

            return (T)result;
        }

        public static void Call(ulong hash, params InputArgument[] arguments)
        {
            MockNativeHost.Dispatch(hash, arguments);
        }
    }
}

namespace RDR2.Math
{
    [StructLayout(LayoutKind.Sequential)]
    public struct Vector3
    {
        public float X;
        public float Y;
        public float Z;

        public Vector3(float x, float y, float z)
        {
            X = x;
            Y = y;
            Z = z;
        }

        public override string ToString()
        {
            return string.Format("({0}, {1}, {2})", X, Y, Z);
        }
    }
}
