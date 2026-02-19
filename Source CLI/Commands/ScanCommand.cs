using System.Diagnostics;
using Src.Models;
using Src.Output;
using Src.Services;

namespace Src.Commands;

public static class ScanCommand
{
    public static async Task<int> ExecuteDirectoryHierarchy(
        string root, string[]? exclude, bool noDefaults, int? timeout, CancellationToken ct)
    {
        var sw = Stopwatch.StartNew();
        using var cts = CreateLinkedCts(timeout, ct);

        try
        {
            if (!Directory.Exists(root))
            {
                WriteError($"Directory not found: {root}");
                return 1;
            }

            var filter = new ExclusionFilter(exclude, noDefaults);
            var scanner = new FileScanner();
            var tree = await scanner.ScanDirectoriesAsync(root, filter, cts.Token);

            sw.Stop();
            var envelope = new OutputEnvelope
            {
                Meta = new MetaInfo { ElapsedMs = sw.ElapsedMilliseconds },
                Tree = tree
            };

            YamlOutputWriter.Write(envelope, Console.Out);
            return 0;
        }
        catch (OperationCanceledException)
        {
            sw.Stop();
            var envelope = new OutputEnvelope
            {
                Meta = new MetaInfo { ElapsedMs = sw.ElapsedMilliseconds, Timeout = true },
                Error = "Operation timed out"
            };
            YamlOutputWriter.Write(envelope, Console.Out);
            return 2;
        }
    }

    public static async Task<int> ExecuteFileListing(
        string root, string[] globs, string[]? exclude, bool noDefaults, int? timeout, CancellationToken ct)
    {
        var sw = Stopwatch.StartNew();
        using var cts = CreateLinkedCts(timeout, ct);

        try
        {
            if (!Directory.Exists(root))
            {
                WriteError($"Directory not found: {root}");
                return 1;
            }

            var filter = new ExclusionFilter(exclude, noDefaults);
            var scanner = new FileScanner();
            var files = await scanner.FindFilesAsync(root, globs, filter, cts.Token);

            sw.Stop();
            var entries = new List<FileEntry>();
            foreach (var f in files)
            {
                entries.Add(new FileEntry
                {
                    Path = PathHelper.GetNormalizedRelativePath(root, f)
                });
            }

            var envelope = new OutputEnvelope
            {
                Meta = new MetaInfo
                {
                    ElapsedMs = sw.ElapsedMilliseconds,
                    FilesScanned = entries.Count,
                    FilesMatched = entries.Count
                },
                Files = entries
            };

            YamlOutputWriter.Write(envelope, Console.Out);
            return 0;
        }
        catch (OperationCanceledException)
        {
            sw.Stop();
            var envelope = new OutputEnvelope
            {
                Meta = new MetaInfo { ElapsedMs = sw.ElapsedMilliseconds, Timeout = true },
                Error = "Operation timed out"
            };
            YamlOutputWriter.Write(envelope, Console.Out);
            return 2;
        }
    }

    private static CancellationTokenSource CreateLinkedCts(int? timeoutSeconds, CancellationToken external)
    {
        var cts = CancellationTokenSource.CreateLinkedTokenSource(external);
        if (timeoutSeconds.HasValue)
            cts.CancelAfter(TimeSpan.FromSeconds(timeoutSeconds.Value));
        return cts;
    }

    private static void WriteError(string message)
    {
        var envelope = new OutputEnvelope { Error = message };
        YamlOutputWriter.Write(envelope, Console.Out);
    }
}
