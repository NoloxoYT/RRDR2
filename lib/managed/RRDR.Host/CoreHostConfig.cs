using System;
using System.Collections.Generic;
using System.IO;

namespace RRDR.Host
{
    /// <summary>
    /// Optional host configuration used by <see cref="CoreHost"/>.
    ///
    /// The ASI loader does not need this file: it scans the mods folder and
    /// loads every core it finds. The file is still supported so a user can
    /// disable a core, override its config path, or change the log file.
    /// </summary>
    internal sealed class CoreHostConfig
    {
        public string ModsDirectory { get; private set; }
        public string LogFile { get; private set; }
        public bool ReloadConfig { get; private set; }
        public HashSet<string> DisabledCores { get; private set; }

        private CoreHostConfig()
        {
            ModsDirectory = null;
            LogFile = "RRDR.Host.log";
            ReloadConfig = true;
            DisabledCores = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        }

        public static CoreHostConfig Load(string path)
        {
            CoreHostConfig config = new CoreHostConfig();

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
                        case "mods_dir":
                            if (value.Length > 0)
                            {
                                config.ModsDirectory = value;
                            }
                            break;
                        case "log_file":
                            if (value.Length > 0)
                            {
                                config.LogFile = value;
                            }
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
                        case "disable":
                            foreach (string name in value.Split(new[] { ',', ';' }, StringSplitOptions.RemoveEmptyEntries))
                            {
                                config.DisabledCores.Add(Path.GetFileName(name.Trim()));
                            }
                            break;
                    }
                }
            }
            catch
            {
                // Invalid config: keep defaults.
            }

            return config;
        }
    }
}
