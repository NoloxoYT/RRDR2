using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using RRDR.Host;

namespace RRDR.Host
{
    /// <summary>
    /// Loads every Rust core found in the mods folder and drives them from one
    /// tick loop. Works with the ScriptHookRDR2DotNet <c>Script</c> shim and
    /// with the native ASI loader.
    /// </summary>
    internal sealed class CoreHost : IDisposable
    {
        private sealed class LoadedCore
        {
            public string Path;
            public RustBridge Bridge;
            public string ConfigPath;
            public DateTime ConfigWriteUtc;
            public bool MissingConfigLogged;
        }

        private readonly string _modsDirectory;
        private readonly CoreHostConfig _config;
        private readonly HostLog _log;
        private readonly List<LoadedCore> _cores = new List<LoadedCore>();
        private readonly Dictionary<string, DateTime> _configWriteTimes =
            new Dictionary<string, DateTime>(StringComparer.OrdinalIgnoreCase);

        private DateTime _nextConfigCheckUtc;
        private long _lastTimestamp;
        private bool _disposed;

        public CoreHost(string modsDirectory, CoreHostConfig config, HostLog log)
        {
            _modsDirectory = modsDirectory;
            _config = config;
            _log = log;
        }

        public int CoreCount
        {
            get { return _cores.Count; }
        }

        public void Load()
        {
            _log.Info("Mods directory: " + _modsDirectory);

            if (!Directory.Exists(_modsDirectory))
            {
                _log.Warn("Mods directory does not exist; no cores loaded");
                return;
            }

            // Windows cores are .dll; Linux builds produce lib*.so for the
            // offline simulator. Scan both so one host works on either OS.
            List<string> candidates = new List<string>();
            candidates.AddRange(Directory.GetFiles(_modsDirectory, "*.dll"));
            candidates.AddRange(Directory.GetFiles(_modsDirectory, "*.so"));
            candidates.Sort(StringComparer.OrdinalIgnoreCase);

            foreach (string path in candidates)
            {
                string name = Path.GetFileName(path);

                if (_config.DisabledCores.Contains(name))
                {
                    _log.Info("Skipping disabled core: " + name);
                    continue;
                }

                if (name.StartsWith("RRDR.Host", StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }

                if (name.StartsWith("ScriptHook", StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }

                LoadCore(path);
            }

            _nextConfigCheckUtc = DateTime.UtcNow.AddSeconds(2.0);
            _lastTimestamp = Stopwatch.GetTimestamp();

            _log.Info("Loaded " + _cores.Count + " core(s)");
        }

        /// <summary>
        /// Cheap pre-filter: managed assemblies and helper DLLs are not cores.
        /// This also avoids probing every managed DLL with LoadLibrary, which
        /// can run module initialisers.
        /// </summary>
        private static bool IsCoreCandidate(string path)
        {
            string name = Path.GetFileName(path);

            if (name.StartsWith("RRDR.", StringComparison.OrdinalIgnoreCase) ||
                name.StartsWith("ScriptHook", StringComparison.OrdinalIgnoreCase) ||
                name.Equals("RRDR.Host.dll", StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            return true;
        }

        private void LoadCore(string path)
        {
            string name = Path.GetFileName(path);

            try
            {
                // Probe first: a managed assembly or a native helper that does
                // not export rd_abi_version is not a core and is skipped.
                if (!IsCoreCandidate(path))
                {
                    return;
                }

                if (!RustBridge.HasBridgeAbi(path))
                {
                    _log.Warn("Skipping (no rd_abi_version export): " + name);
                    return;
                }

                HostConfig perCore = HostConfig.ForCore(path);
                HostLog coreLog = _log;

                string configName = HostConfig.ResolvePath(
                    _modsDirectory,
                    perCore.CoreConfig);

                RustBridge bridge = new RustBridge(_modsDirectory, perCore, coreLog);

                LoadedCore core = new LoadedCore
                {
                    Path = path,
                    Bridge = bridge,
                    ConfigPath = configName,
                    MissingConfigLogged = false
                };

                if (!string.IsNullOrEmpty(configName) && File.Exists(configName))
                {
                    core.ConfigWriteUtc = File.GetLastWriteTimeUtc(configName);
                }

                _cores.Add(core);
                _log.Info("Loaded core: " + name);
            }
            catch (Exception exception)
            {
                _log.Error("Failed to load core " + name + ": " + exception);
            }
        }

        public void Tick(float deltaSeconds)
        {
            if (_disposed)
            {
                return;
            }

            CheckConfigReload();

            for (int index = 0; index < _cores.Count; index++)
            {
                LoadedCore core = _cores[index];

                if (core.Bridge == null || !core.Bridge.IsInitialized)
                {
                    continue;
                }

                try
                {
                    core.Bridge.Tick(deltaSeconds);
                }
                catch (Exception exception)
                {
                    _log.Error("Unhandled exception while ticking " +
                               Path.GetFileName(core.Path) + ": " + exception);
                }
            }
        }

        private void CheckConfigReload()
        {
            if (!_config.ReloadConfig)
            {
                return;
            }

            DateTime now = DateTime.UtcNow;
            if (now < _nextConfigCheckUtc)
            {
                return;
            }

            _nextConfigCheckUtc = now.AddSeconds(2.0);

            foreach (LoadedCore core in _cores)
            {
                if (string.IsNullOrEmpty(core.ConfigPath) || core.Bridge == null)
                {
                    continue;
                }

                try
                {
                    if (!File.Exists(core.ConfigPath))
                    {
                        if (!core.MissingConfigLogged)
                        {
                            _log.Warn("Core config not found: " + core.ConfigPath);
                            core.MissingConfigLogged = true;
                        }
                        continue;
                    }

                    core.MissingConfigLogged = false;
                    DateTime writeTime = File.GetLastWriteTimeUtc(core.ConfigPath);
                    if (writeTime == core.ConfigWriteUtc)
                    {
                        continue;
                    }

                    core.ConfigWriteUtc = writeTime;
                    if (core.Bridge.Reload(core.ConfigPath))
                    {
                        _log.Info("Core config reloaded: " +
                                  Path.GetFileName(core.Path));
                    }
                }
                catch (Exception exception)
                {
                    _log.Error("Config reload check failed for " +
                               Path.GetFileName(core.Path) + ": " + exception);
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

            foreach (LoadedCore core in _cores)
            {
                if (core.Bridge == null)
                {
                    continue;
                }

                try
                {
                    core.Bridge.Dispose();
                }
                catch (Exception exception)
                {
                    _log.Error("Failed to unload " + Path.GetFileName(core.Path) +
                               ": " + exception);
                }
            }

            _cores.Clear();
        }
    }
}
