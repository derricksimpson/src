using System.Collections.Concurrent;
using Src.Models;

namespace Src.Services;

public sealed class FileScanner
{
    private static readonly HashSet<string> SourceExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".cs", ".ts", ".tsx", ".js", ".jsx", ".py", ".rb", ".go", ".rs",
        ".java", ".kt", ".scala", ".swift", ".m", ".mm", ".c", ".cpp",
        ".cc", ".cxx", ".h", ".hpp", ".hxx", ".lua", ".pl", ".pm",
        ".php", ".r", ".dart", ".ex", ".exs", ".erl", ".hs", ".fs",
        ".fsx", ".fsi", ".ml", ".mli", ".v", ".sv", ".vhd", ".vhdl",
        ".sql", ".sh", ".bash", ".zsh", ".ps1", ".psm1", ".bat", ".cmd",
        ".yaml", ".yml", ".json", ".xml", ".html", ".htm", ".css",
        ".scss", ".sass", ".less", ".vue", ".svelte", ".astro",
        ".md", ".mdx", ".rst", ".txt", ".toml", ".ini", ".cfg",
        ".conf", ".env", ".dockerfile", ".tf", ".tfvars", ".hcl",
        ".proto", ".graphql", ".gql", ".razor", ".cshtml", ".csproj",
        ".sln", ".gradle", ".cmake", ".makefile", ".mk"
    };

    private static readonly EnumerationOptions FastEnumOptions = new()
    {
        IgnoreInaccessible = true,
        AttributesToSkip = FileAttributes.System
    };

    public async Task<ScanResult> ScanDirectoriesAsync(
        string root, ExclusionFilter filter, CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();
        var dirInfo = new DirectoryInfo(root);
        var children = new ConcurrentQueue<ScanResult>();
        var files = new List<string>();

        try
        {
            foreach (var file in dirInfo.EnumerateFiles("*", FastEnumOptions))
            {
                if (IsSourceFile(file.Name))
                    files.Add(file.Name);
            }

            var subdirs = new List<DirectoryInfo>();
            foreach (var d in dirInfo.EnumerateDirectories("*", FastEnumOptions))
            {
                if (!filter.IsExcluded(d.Name))
                    subdirs.Add(d);
            }

            await Parallel.ForEachAsync(subdirs, ct, async (subdir, token) =>
            {
                var child = await ScanDirectoriesAsync(subdir.FullName, filter, token);
                if (child.Files?.Count > 0 || child.Children?.Count > 0)
                    children.Enqueue(child);
            });
        }
        catch (UnauthorizedAccessException) { }
        catch (IOException) { }

        var sortedChildren = new List<ScanResult>(children);
        sortedChildren.Sort(static (a, b) => string.Compare(a.Name, b.Name, StringComparison.OrdinalIgnoreCase));
        files.Sort(StringComparer.OrdinalIgnoreCase);

        return new ScanResult
        {
            Name = dirInfo.Name,
            Children = sortedChildren.Count > 0 ? sortedChildren : null,
            Files = files.Count > 0 ? files : null
        };
    }

    public async Task<List<string>> FindFilesAsync(
        string root, IReadOnlyList<string> globs, ExclusionFilter filter, CancellationToken ct)
    {
        var results = new ConcurrentQueue<string>();
        await ScanForFilesRecursive(root, globs, filter, results, ct);
        var list = new List<string>(results);
        list.Sort(StringComparer.OrdinalIgnoreCase);
        return list;
    }

    private async Task ScanForFilesRecursive(
        string directory, IReadOnlyList<string> globs, ExclusionFilter filter,
        ConcurrentQueue<string> results, CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();

        try
        {
            foreach (var file in Directory.EnumerateFiles(directory, "*", FastEnumOptions))
            {
                var fileName = Path.GetFileName(file.AsSpan());
                if (!filter.IsFileExcluded(fileName) && GlobMatcher.MatchesAny(fileName, globs))
                    results.Enqueue(file);
            }

            var subdirs = new List<string>();
            foreach (var d in Directory.EnumerateDirectories(directory, "*", FastEnumOptions))
            {
                var dirName = Path.GetFileName(d.AsSpan());
                if (!filter.IsExcluded(dirName))
                    subdirs.Add(d);
            }

            await Parallel.ForEachAsync(subdirs, ct, async (subdir, token) =>
            {
                await ScanForFilesRecursive(subdir, globs, filter, results, token);
            });
        }
        catch (UnauthorizedAccessException) { }
        catch (IOException) { }
    }

    private static bool IsSourceFile(string fileName) =>
        SourceExtensions.Contains(Path.GetExtension(fileName));
}
