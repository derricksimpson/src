namespace Src.Services;

public sealed class ExclusionFilter
{
    private static readonly HashSet<string> DefaultExclusions = new(StringComparer.OrdinalIgnoreCase)
    {
        "node_modules", ".git", "bin", "obj", "dist", ".vs",
        "__pycache__", ".idea", ".vscode", ".svn", ".hg",
        "coverage", ".next", ".nuxt", "target", "build",
        "packages", ".cache", ".output", ".parcel-cache"
    };

    private readonly HashSet<string> _exclusions;
    private readonly HashSet<string>.AlternateLookup<ReadOnlySpan<char>> _spanLookup;

    public ExclusionFilter(IReadOnlyList<string>? additionalExclusions, bool disableDefaults)
    {
        _exclusions = disableDefaults
            ? new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            : new HashSet<string>(DefaultExclusions, StringComparer.OrdinalIgnoreCase);

        if (additionalExclusions is not null)
        {
            foreach (var pattern in additionalExclusions)
                _exclusions.Add(pattern);
        }

        _spanLookup = _exclusions.GetAlternateLookup<ReadOnlySpan<char>>();
    }

    public bool IsExcluded(string directoryName) =>
        _exclusions.Contains(directoryName);

    public bool IsExcluded(ReadOnlySpan<char> directoryName) =>
        _spanLookup.Contains(directoryName);

    public bool IsFileExcluded(string fileName) =>
        _exclusions.Contains(fileName);

    public bool IsFileExcluded(ReadOnlySpan<char> fileName) =>
        _spanLookup.Contains(fileName);
}
