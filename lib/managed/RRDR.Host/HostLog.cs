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
    /// <remarks>
    /// The file handle is kept open and written through per line (auto-flush).
    /// The previous implementation called File.AppendAllText per message, which
    /// pays a CreateFile/WriteFile/CloseHandle round trip per line - a mod that
    /// logs once per tick was doing disk I/O every frame. A mod that logs every
    /// frame still costs one WriteFile per line, so keep per-tick logging out
    /// of hot loops. The log is capped at 16 MB: when the cap is passed at
    /// open time the file is truncated and starts fresh.
    /// </remarks>
    internal sealed class HostLog : IDisposable
    {
        private const long MaxBytes = 16 * 1024 * 1024;

        private readonly object _sync = new object();
        private readonly string _path;
        private StreamWriter _writer;
        private bool _disposed;

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

        /// <summary>Flushes and closes the log. Idempotent.</summary>
        public void Dispose()
        {
            lock (_sync)
            {
                if (_disposed)
                {
                    return;
                }

                _disposed = true;
                if (_writer != null)
                {
                    try
                    {
                        _writer.Flush();
                        _writer.Dispose();
                    }
                    catch (Exception exception)
                    {
                        Debug.WriteLine("RRDR.Host log close failure: " + exception);
                    }

                    _writer = null;
                }
            }
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
                if (_disposed)
                {
                    return;
                }

                try
                {
                    if (_writer == null)
                    {
                        _writer = Open();
                    }

                    _writer.Write(line);
                    _writer.Flush();
                }
                catch (Exception exception)
                {
                    // A broken log must never take the game down; drop the
                    // writer so the next line retries from a clean state.
                    Debug.WriteLine("RRDR.Host log failure: " + exception);
                    try
                    {
                        _writer?.Dispose();
                    }
                    catch
                    {
                        // Nothing left to do about the log.
                    }

                    _writer = null;
                }
            }
        }

        private StreamWriter Open()
        {
            string directory = Path.GetDirectoryName(_path);
            if (!string.IsNullOrEmpty(directory) && !Directory.Exists(directory))
            {
                Directory.CreateDirectory(directory);
            }

            // Cap: a runaway mod would otherwise grow the log without bound
            // over a long session. Truncating keeps the newest lines; the old
            // content is diagnostic, not a record.
            if (File.Exists(_path))
            {
                try
                {
                    if (new FileInfo(_path).Length > MaxBytes)
                    {
                        File.Delete(_path);
                    }
                }
                catch
                {
                    // If the file cannot be stat'd or deleted, keep appending.
                }
            }

            var stream = new FileStream(_path, FileMode.Append, FileAccess.Write, FileShare.Read);
            return new StreamWriter(stream, new UTF8Encoding(false)) { AutoFlush = true };
        }
    }
}
