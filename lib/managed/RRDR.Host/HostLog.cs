using System;
using System.Diagnostics;
using System.IO;
using System.Text;

namespace RRDR.Host
{
    /// <summary>
    /// Thread-safe file logger. The game does not expose a public console, so
    /// everything goes to a file next to the host DLL.
    /// </summary>
    internal sealed class HostLog
    {
        private readonly object _sync = new object();
        private readonly string _path;

        public HostLog(string path)
        {
            _path = path;
        }

        public void Info(string message)
        {
            Write("INFO", message);
        }

        public void Warn(string message)
        {
            Write("WARN", message);
        }

        public void Error(string message)
        {
            Write("ERROR", message);
        }

        public void Rust(uint level, string message)
        {
            string name;
            switch (level)
            {
                case 0: name = "ERROR"; break;
                case 1: name = "WARN"; break;
                case 2: name = "INFO"; break;
                default: name = "DEBUG"; break;
            }
            Write(name, message);
        }

        private void Write(string level, string message)
        {
            string line = string.Format(
                "[{0:yyyy-MM-dd HH:mm:ss.fff}] [{1}] {2}{3}",
                DateTime.Now,
                level,
                message,
                Environment.NewLine);

            lock (_sync)
            {
                try
                {
                    string directory = Path.GetDirectoryName(_path);
                    if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
                    {
                        Directory.CreateDirectory(directory);
                    }

                    File.AppendAllText(_path, line, Encoding.UTF8);
                }
                catch (Exception exception)
                {
                    // A broken log must never take the game down.
                    Debug.WriteLine("RRDR.Host log failure: " + exception);
                }
            }
        }
    }
}
