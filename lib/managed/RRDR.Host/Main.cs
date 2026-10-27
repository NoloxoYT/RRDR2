using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using RDR2;

namespace RRDR.Host
{
    /// <summary>
    /// The managed script ScriptHookRDR2DotNet sees. It scans the Rscripts
    /// folder, loads every Rust core it finds and forwards Tick/Aborted plus
    /// the native-call callback. There is no gameplay code here.
    /// </summary>
    public sealed class Main : Script
    {
        private readonly string _baseDirectory;
        private readonly HostLog _log;
        private readonly CoreHost _cores;
        private long _lastTimestamp;

        public Main()
        {
            // ScriptHookRDR2DotNet shadow-copies managed assemblies, so
            // Assembly.Location can point into %TEMP%. The AppDomain base
            // directory is explicitly set to the game's scripts folder.
            _baseDirectory = ResolveBaseDirectory();

            CoreHostConfig coreConfig = CoreHostConfig.Load(
                Path.Combine(_baseDirectory, "RRDR.Host.ini"));

            string logPath = HostConfig.ResolvePath(_baseDirectory, coreConfig.LogFile);
            _log = new HostLog(logPath);

            _log.Info("RRDR.Host starting");
            _log.Info("Script file: " + (string.IsNullOrEmpty(FileName) ? "(unknown)" : FileName));
            _log.Info("Base directory: " + _baseDirectory);
            _log.Info("ScriptHook API: " + typeof(Script).Assembly.FullName);
#if REAL_API
            _log.Info("Build: real ScriptHookRDRNetAPI reference");
#else
            _log.Info("Build: mock ScriptHookRDRNetAPI reference (offline)");
#endif

            try
            {
                string modsDirectory = string.IsNullOrEmpty(coreConfig.ModsDirectory)
                    ? _baseDirectory
                    : HostConfig.ResolvePath(_baseDirectory, coreConfig.ModsDirectory);

                _cores = new CoreHost(modsDirectory, coreConfig, _log);
                _cores.Load();

                _lastTimestamp = Stopwatch.GetTimestamp();

                Tick += OnTick;
                Aborted += OnAborted;
                Interval = 0;

                _log.Info("Host initialized with " + _cores.CoreCount + " core(s)");
            }
            catch (Exception exception)
            {
                _log.Error("Failed to initialize cores: " + exception);
                _cores?.Dispose();
            }
        }

        private string ResolveBaseDirectory()
        {
            // Script.FileName is set by ScriptHookRDR2DotNet to the original
            // assembly path, even when the AppDomain shadow-copies the DLL.
            if (!string.IsNullOrEmpty(FileName))
            {
                string scriptDirectory = Path.GetDirectoryName(FileName);
                if (!string.IsNullOrEmpty(scriptDirectory) && Directory.Exists(scriptDirectory))
                {
                    return scriptDirectory;
                }
            }

            string domainBase = AppDomain.CurrentDomain.BaseDirectory;
            if (!string.IsNullOrEmpty(domainBase) && Directory.Exists(domainBase))
            {
                return domainBase;
            }

            string assemblyLocation = Assembly.GetExecutingAssembly().Location;
            if (!string.IsNullOrEmpty(assemblyLocation))
            {
                return Path.GetDirectoryName(assemblyLocation) ?? Environment.CurrentDirectory;
            }

            return Environment.CurrentDirectory;
        }

        private void OnTick(object sender, EventArgs eventArgs)
        {
            if (_cores == null)
            {
                return;
            }

            long now = Stopwatch.GetTimestamp();
            float deltaSeconds = (float)((now - _lastTimestamp) / (double)Stopwatch.Frequency);
            _lastTimestamp = now;

            // Clamp on both ends, like the native ASI loader: negatives mean a
            // clock glitch, and after a loading screen, pause menu or alt-tab
            // the first tick would otherwise hand the cores a delta of tens of
            // seconds - any dt-based logic (bleed timers, smoothing) jumps.
            if (deltaSeconds < 0.0f)
            {
                deltaSeconds = 0.0f;
            }
            else if (deltaSeconds > 0.25f)
            {
                deltaSeconds = 0.25f;
            }

            TickOnce(deltaSeconds);
        }

        /// <summary>
        /// Tick entry point used by the game and by the deterministic simulator.
        /// </summary>
        internal void TickOnce(float deltaSeconds)
        {
            if (_cores == null)
            {
                return;
            }

            _cores.Tick(deltaSeconds);
        }

        private void OnAborted(object sender, EventArgs eventArgs)
        {
            _log.Info("Host aborted");
            _cores?.Dispose();
            _log.Dispose();
        }

    }
}
