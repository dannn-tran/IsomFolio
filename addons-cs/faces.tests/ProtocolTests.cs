using System.Text.Json;

namespace IsomFolio.Addons.Faces.Tests;

public class ProtocolTests
{
    [Fact]
    public void SendHello_EmitsValidJson()
    {
        var sw = new StringWriter();
        Protocol.SendHello(sw, ["cluster_faces", "detect"]);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("hello", root.GetProperty("type").GetString());
        Assert.Equal(1, root.GetProperty("protocol_version").GetInt32());
        Assert.Equal(1, root.GetProperty("addon_api_version").GetInt32());
        var caps = root.GetProperty("capabilities").EnumerateArray().Select(e => e.GetString()).ToList();
        Assert.Contains("cluster_faces", caps);
        Assert.Contains("detect", caps);
    }

    [Fact]
    public void SendError_EmitsIdAndError()
    {
        var sw = new StringWriter();
        Protocol.SendError(sw, 42, "something broke");
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal(42UL, root.GetProperty("id").GetUInt64());
        Assert.Equal("something broke", root.GetProperty("error").GetString());
    }

    [Fact]
    public void EmitLog_UsesSnakeCaseKeys()
    {
        var sw = new StringWriter();
        Protocol.EmitLog(sw, "info", "test message");
        var raw = sw.ToString().Trim();

        Assert.Contains("\"type\":", raw);
        Assert.Contains("\"level\":", raw);
        Assert.Contains("\"message\":", raw);
        Assert.DoesNotContain("\"Type\":", raw);
    }

    [Fact]
    public void EmitProgress_IncludesIdAndPercent()
    {
        var sw = new StringWriter();
        Protocol.EmitProgress(sw, 7, 45);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("progress", root.GetProperty("type").GetString());
        Assert.Equal(7UL, root.GetProperty("id").GetUInt64());
        Assert.Equal(45, root.GetProperty("percent").GetInt32());
    }

    [Fact]
    public void ParseRequest_ValidJson()
    {
        var req = Protocol.ParseRequest("{\"id\":1,\"method\":\"cluster_faces\",\"params\":{\"files\":[]}}");
        Assert.NotNull(req);
        Assert.Equal(1UL, req!.Id);
        Assert.Equal("cluster_faces", req.Method);
    }

    [Fact]
    public void ParseRequest_InvalidJson_ReturnsNull()
    {
        Assert.Null(Protocol.ParseRequest("not json"));
    }

    [Fact]
    public void SendResponse_EmitsClusterResultWithSnakeCase()
    {
        var sw = new StringWriter();
        var result = new ClusterResult(
            [new ClusterEntry("face-abc", [new FaceMember("file1", new BboxData(0.1f, 0.2f, 0.3f, 0.4f))])],
            [new FaceMember("file2", new BboxData(0.5f, 0.6f, 0.1f, 0.1f))]
        );
        Protocol.SendResponse(sw, 1, result);
        var raw = sw.ToString().Trim();

        Assert.Contains("\"id\":1", raw);
        Assert.Contains("\"clusters\":", raw);
        Assert.Contains("\"noise\":", raw);
        Assert.Contains("\"file_id\":\"file1\"", raw);
        Assert.Contains("\"bbox\":", raw);
    }
}
