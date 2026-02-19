using System.IO.Enumeration;

namespace Src.Services;

public static class GlobMatcher
{
    public static bool Matches(ReadOnlySpan<char> fileName, ReadOnlySpan<char> pattern) =>
        FileSystemName.MatchesSimpleExpression(pattern, fileName, ignoreCase: true);

    public static bool MatchesAny(ReadOnlySpan<char> fileName, IReadOnlyList<string> patterns)
    {
        for (int i = 0; i < patterns.Count; i++)
        {
            if (FileSystemName.MatchesSimpleExpression(patterns[i].AsSpan(), fileName, ignoreCase: true))
                return true;
        }
        return false;
    }
}
