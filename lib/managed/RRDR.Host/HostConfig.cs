using System;
using System.IO;

namespace RRDR.Host
{
    internal sealed class HostConfig
    {
        public string CoreDll { get; private set; }
        public string CoreConfig { get; private set; }
        public string LogFile { get; private set; }
        public bool ReloadConfig { get; private set; }

        private HostConfig()
        {
            CoreDll = "RealisticDamagesCore.dll";
            CoreConfig = "RealisticDamages.ini";
            LogFile = "RRDR.Host.log";
            ReloadConfig = true;
        }

        public static HostConfig Load(string path)
        {
            HostConfig config = new HostConfig();

            try
            {
                if (!File.Exists(path))
                {
                    return config;
                }

                foreach (string rawLine in File.ReadAllLines(path))
                {
                    string line = rawLine.TrimStart('\uFEFF');
                    int comment = line.IndexOf('#');
                    if (comment >= 0)
                    {
                        line = line.Substring(0, comment);
                    }
                    comment = line.IndexOf(';');
                    if (comment >= 0)
                    {
                        line = line.Substring(0, comment);
                    }

                    line = line.Trim();
                    if (line.Length == 0 || (line[0] == '[' && line[line.Length - 1] == ']'))
                    {
                        continue;
                    }

                    int equals = line.IndexOf('=');
                    if (equals <= 0)
                    {
                        continue;
                    }

                    string key = line.Substring(0, equals).Trim();
                    string value = line.Substring(equals + 1).Trim();

                    switch (key.ToLowerInvariant())
                    {
                        case "core_dll":
                            if (value.Length > 0) config.CoreDll = value;
                            break;
                        case "core_config":
                            config.CoreConfig = value;
                            break;
                        case "log_file":
                            if (value.Length > 0) config.LogFile = value;
                            break;
                        case "reload_config":
                            bool parsed;
                            if (bool.TryParse(value, out parsed))
                            {
                                config.ReloadConfig = parsed;
                            }
                            else if (value == "1")
                            {
                                config.ReloadConfig = true;
                            }
                            else if (value == "0")
                            {
                                config.ReloadConfig = false;
                            }
                            break;
                    }
                }
            }
            catch
            {
                // Invalid host config: fall back to defaults.
            }

            return config;
        }

        public static string ResolvePath(string baseDirectory, string value)
        {
            if (string.IsNullOrEmpty(value))
            {
                return null;
            }

            string expanded = Environment.ExpandEnvironmentVariables(value);
            return Path.IsPathRooted(expanded)
                ? Path.GetFullPath(expanded)
                : Path.GetFullPath(Path.Combine(baseDirectory, expanded));
        }

        /// <summary>
        /// Per-core defaults used by <see cref="CoreHost"/>. The config name is
        /// derived from the DLL name: Foo.dll looks for Foo.ini.
        /// </summary>
        /// <remarks>
        /// Only <c>RRDR.Host.ini</c> governs host behaviour: a per-core ini is
        /// the core's own gameplay config and is passed to the core verbatim
        /// (the core reloads it itself when <c>reload_config</c> is on). Host
        /// keys such as <c>disabled_cores</c> or <c>log_file</c> are not read
        /// from per-core files.
        /// </remarks>
        public static HostConfig ForCore(string corePath)
        {
            HostConfig config = new HostConfig();
            string name = Path.GetFileNameWithoutExtension(corePath);

            config.CoreDll = Path.GetFileName(corePath);
            config.CoreConfig = name + ".ini";
            config.LogFile = "RRDR.Host.log";
            config.ReloadConfig = true;
            return config;
        }
    }
}
