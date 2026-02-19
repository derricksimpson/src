using System.Collections.Concurrent;
using System.Text;
using System.Text.RegularExpressions;
using Src.Models;

namespace Src.Services;

public sealed class ContentSearcher
{
    private const int BufferSize = 64 * 1024;
    private const int BinaryCheckSize = 8192;

    public async Task<List<FileEntry>> SearchAsync(
        IReadOnlyList<string> filePaths,
        string rootPath,
        string pattern,
        bool isRegex,
        int padLines,
        CancellationToken ct)
    {
        var matcher = BuildMatcher(pattern, isRegex);
        var results = new ConcurrentQueue<FileEntry>();
        var maxConcurrency = Math.Max(1, Environment.ProcessorCount * 2);

        await Parallel.ForEachAsync(filePaths,
            new ParallelOptions { MaxDegreeOfParallelism = maxConcurrency, CancellationToken = ct },
            (filePath, token) =>
            {
                token.ThrowIfCancellationRequested();
                var entry = ProcessFile(filePath, rootPath, matcher, padLines);
                if (entry is not null)
                    results.Enqueue(entry);
                return ValueTask.CompletedTask;
            });

        var list = new List<FileEntry>(results);
        list.Sort(static (a, b) => string.Compare(a.Path, b.Path, StringComparison.OrdinalIgnoreCase));
        return list;
    }

    private static FileEntry? ProcessFile(string filePath, string rootPath, IMatcher matcher, int padLines)
    {
        try
        {
            using var fs = new FileStream(filePath, FileMode.Open, FileAccess.Read,
                FileShare.Read, BufferSize, FileOptions.SequentialScan);

            if (fs.Length == 0)
                return null;

            if (IsBinaryStream(fs))
                return null;

            fs.Position = 0;

            using var reader = new StreamReader(fs, Encoding.UTF8,
                detectEncodingFromByteOrderMarks: true, bufferSize: BufferSize, leaveOpen: true);

            var matchingLineIndices = new List<int>();
            var lineCount = 0;
            string? line;

            while ((line = reader.ReadLine()) is not null)
            {
                if (matcher.IsMatch(line.AsSpan()))
                    matchingLineIndices.Add(lineCount);
                lineCount++;
            }

            if (matchingLineIndices.Count == 0)
                return null;

            var ranges = MergeRanges(matchingLineIndices, padLines, lineCount);

            fs.Position = 0;
            using var reader2 = new StreamReader(fs, Encoding.UTF8,
                detectEncodingFromByteOrderMarks: true, bufferSize: BufferSize, leaveOpen: true);

            var chunks = BuildChunksFromRanges(reader2, ranges, lineCount);
            var relativePath = PathHelper.GetNormalizedRelativePath(rootPath, filePath);

            if (chunks.Count == 1 && chunks[0].StartLine == 1 && chunks[0].EndLine == lineCount)
            {
                return new FileEntry
                {
                    Path = relativePath,
                    Contents = chunks[0].Content
                };
            }

            return new FileEntry
            {
                Path = relativePath,
                Chunks = chunks
            };
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException)
        {
            var relativePath = PathHelper.GetNormalizedRelativePath(rootPath, filePath);
            return new FileEntry
            {
                Path = relativePath,
                Error = ex.Message
            };
        }
    }

    private static bool IsBinaryStream(FileStream fs)
    {
        Span<byte> buffer = stackalloc byte[Math.Min(BinaryCheckSize, (int)Math.Min(fs.Length, BinaryCheckSize))];
        int bytesRead = fs.Read(buffer);
        return buffer[..bytesRead].Contains((byte)0);
    }

    private static List<(int Start, int End)> MergeRanges(List<int> matchingIndices, int pad, int lineCount)
    {
        var ranges = new List<(int Start, int End)>(matchingIndices.Count);

        foreach (var idx in matchingIndices)
        {
            int start = Math.Max(0, idx - pad);
            int end = Math.Min(lineCount - 1, idx + pad);

            if (ranges.Count > 0 && start <= ranges[^1].End + 1)
            {
                ranges[^1] = (ranges[^1].Start, Math.Max(ranges[^1].End, end));
            }
            else
            {
                ranges.Add((start, end));
            }
        }

        return ranges;
    }

    private static List<FileChunk> BuildChunksFromRanges(
        StreamReader reader, List<(int Start, int End)> ranges, int lineCount)
    {
        var chunks = new List<FileChunk>(ranges.Count);
        var sb = new StringBuilder();
        int currentLine = 0;
        int rangeIdx = 0;

        while (rangeIdx < ranges.Count)
        {
            var (start, end) = ranges[rangeIdx];

            while (currentLine < start)
            {
                reader.ReadLine();
                currentLine++;
            }

            sb.Clear();
            while (currentLine <= end)
            {
                var line = reader.ReadLine();
                if (line is null) break;

                int lineNum = currentLine + 1;
                sb.Append(lineNum);
                sb.Append(".  ");
                sb.AppendLine(line);
                currentLine++;
            }

            chunks.Add(new FileChunk
            {
                StartLine = start + 1,
                EndLine = Math.Min(end + 1, lineCount),
                Content = sb.ToString()
            });

            rangeIdx++;
        }

        return chunks;
    }

    private static IMatcher BuildMatcher(string pattern, bool isRegex)
    {
        if (isRegex)
            return new RegexMatcher(pattern);

        if (pattern.Contains('|'))
        {
            var terms = pattern.Split('|', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
            return new MultiTermMatcher(terms);
        }

        return new LiteralMatcher(pattern);
    }

    private interface IMatcher
    {
        bool IsMatch(ReadOnlySpan<char> line);
    }

    private sealed class LiteralMatcher : IMatcher
    {
        private readonly string _pattern;

        public LiteralMatcher(string pattern) => _pattern = pattern;

        public bool IsMatch(ReadOnlySpan<char> line) =>
            line.Contains(_pattern.AsSpan(), StringComparison.OrdinalIgnoreCase);
    }

    private sealed class MultiTermMatcher : IMatcher
    {
        private readonly string[] _terms;

        public MultiTermMatcher(string[] terms) => _terms = terms;

        public bool IsMatch(ReadOnlySpan<char> line)
        {
            foreach (var term in _terms)
            {
                if (line.Contains(term.AsSpan(), StringComparison.OrdinalIgnoreCase))
                    return true;
            }
            return false;
        }
    }

    private sealed class RegexMatcher : IMatcher
    {
        private readonly Regex _regex;

        public RegexMatcher(string pattern) =>
            _regex = new Regex(pattern, RegexOptions.Compiled | RegexOptions.IgnoreCase);

        public bool IsMatch(ReadOnlySpan<char> line) => _regex.IsMatch(line);
    }
}
