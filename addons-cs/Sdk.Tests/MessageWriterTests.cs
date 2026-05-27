using System.Text.Json;

namespace IsomFolio.Addons.Sdk.Tests;

public class MessageWriterTests
{
    [Fact]
    public async Task SendHandshakeResponse_EmitsTypeOkAndCapabilities()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.SendHandshakeResponseAsync(1, "2.0.0", [AddonCapability.ClusterFaces, AddonCapability.Classify]);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("ok", root.GetProperty("type").GetString());
        Assert.Equal(1UL, root.GetProperty("id").GetUInt64());
        var result = root.GetProperty("result");
        Assert.Equal(1, result.GetProperty("protocol_version").GetInt32());
        Assert.Equal("2.0.0", result.GetProperty("addon_version").GetString());
        var caps = result.GetProperty("capabilities").EnumerateArray().Select(e => e.GetString()).ToList();
        Assert.Contains("cluster_faces", caps);
        Assert.Contains("classify", caps);
    }

    [Fact]
    public async Task SendPingResponse_EmitsTypeOkAndEmptyResult()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.SendPingResponseAsync(7);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("ok", root.GetProperty("type").GetString());
        Assert.Equal(7UL, root.GetProperty("id").GetUInt64());
        Assert.Equal(JsonValueKind.Object, root.GetProperty("result").ValueKind);
    }

    [Fact]
    public async Task SendReady_EmitsTypeReady()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.SendReadyAsync();
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("ready", root.GetProperty("type").GetString());
        Assert.Single(root.EnumerateObject());
    }

    [Fact]
    public async Task SendErrorResponse_EmitsTypeErrorAndIdAndError()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.SendErrorResponseAsync(42, "something broke");
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("error", root.GetProperty("type").GetString());
        Assert.Equal(42UL, root.GetProperty("id").GetUInt64());
        Assert.Equal("something broke", root.GetProperty("error").GetString());
    }

    [Fact]
    public async Task Log_UsesSnakeCaseKeys()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.LogAsync(LogLevel.Info, "test message");
        var raw = sw.ToString().Trim();

        Assert.Contains("\"type\":\"log\"", raw);
        Assert.Contains("\"level\":\"info\"", raw);
        Assert.Contains("\"message\":\"test message\"", raw);
    }

    [Fact]
    public async Task SendProgress_EmitsTypeAndIdAndPercent()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        await writer.SendProgressAsync(7, 45);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("progress", root.GetProperty("type").GetString());
        Assert.Equal(7UL, root.GetProperty("id").GetUInt64());
        Assert.Equal(45, root.GetProperty("percent").GetInt32());
    }

    [Fact]
    public async Task SendClusterResponse_EmitsTypeOkAndResult()
    {
        var sw = new StringWriter();
        var writer = new MessageWriter(sw);
        var result = new ClusterResult(
            [new ClusterEntry("face-abc", [new FaceMember("file1", new BboxData(0.1f, 0.2f, 0.3f, 0.4f))])],
            [new FaceMember("file2", new BboxData(0.5f, 0.6f, 0.1f, 0.1f))]
        );
        await writer.SendClusterResponseAsync(1, result);
        var root = JsonDocument.Parse(sw.ToString().Trim()).RootElement;

        Assert.Equal("ok", root.GetProperty("type").GetString());
        Assert.Equal(1UL, root.GetProperty("id").GetUInt64());
        var r = root.GetProperty("result");
        Assert.True(r.TryGetProperty("clusters", out _));
        Assert.True(r.TryGetProperty("noise", out _));
    }
}
