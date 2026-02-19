namespace Src.Services;

public static class PathHelper
{
    public static string GetNormalizedRelativePath(string root, string fullPath)
    {
        var relative = Path.GetRelativePath(root, fullPath);
        if (!relative.AsSpan().Contains('\\'))
            return relative;

        return string.Create(relative.Length, relative, static (span, rel) =>
        {
            rel.AsSpan().CopyTo(span);
            span.Replace('\\', '/');
        });
    }
}
