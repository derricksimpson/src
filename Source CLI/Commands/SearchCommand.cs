using System.Diagnostics;
using System.Text.RegularExpressions;
using Src.Models;
using Src.Output;
using Src.Services;

namespace Src.Commands;

public static class SearchCommand
{
    public static async Task<int> ExecuteAsync(
        string root, string[]? globs, string pattern, bool isRegex,
        int pad, string[]? exclude, bool noDefaults, int? timeout, CancellationToken ct)
    {
        if (isRegex)
        {
            try { _ = new Regex(pattern); }
            catch (RegexParseException ex)
            {
                WriteError($"Invalid regex: {ex.Message}");
                return 1;
            }
        }

        var sw = Stopwatch.StartNew();
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        if (timeout.HasValue)
            cts.CancelAfter(TimeSpan.FromSeconds(timeout.Value));

        try
        {
            if (!Directory.Exists(root))
            {
                WriteError($"Directory not found: {root}");
                return 1;
            }

            var filter = new ExclusionFilter(exclude, noDefaults);
            var scanner = new FileScanner();
            var searcher = new ContentSearcher();

            IReadOnlyList<string> candidateFiles;
            if (globs is { Length: > 0 })
            {
                candidateFiles = await scanner.FindFilesAsync(root, globs, filter, cts.Token);
            }
            else
            {
                candidateFiles = await scanner.FindFilesAsync(root, new[] { "*.*" }, filter, cts.Token);
            }

            var entries = await searcher.SearchAsync(candidateFiles, root, pattern, isRegex, pad, cts.Token);

            sw.Stop();
            var envelope = new OutputEnvelope
            {
                Meta = new MetaInfo
                {
                    ElapsedMs = sw.ElapsedMilliseconds,
                    FilesScanned = candidateFiles.Count,
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
                Error = "Operation timed out — partial results may be incomplete"
            };
            YamlOutputWriter.Write(envelope, Console.Out);
            return 2;
        }
    }

    private static void WriteError(string message)
    {
        var envelope = new OutputEnvelope { Error = message };
        YamlOutputWriter.Write(envelope, Console.Out);
    }
}
