using System;
using System.IO;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Logging;

namespace sts_sim_bridge_mod;

/// <summary>
/// Persistent TCP client for python/sts_sim/server.py's line-delimited JSON
/// protocol. Connects lazily and reconnects on failure; if the server isn't
/// running, logs once and skips pushes rather than spamming or crashing combat.
/// </summary>
public static class AnalysisClient
{
    private const string Host = "127.0.0.1";
    private const int Port = 8765;

    private static TcpClient? _client;
    private static bool _loggedConnectFailure;

    /// Sends one line-delimited JSON request and returns the response line,
    /// or null if the server is unreachable. Safe to call from a background
    /// thread (does its own socket I/O).
    public static async Task<string?> SendAnalyzeRequestAsync(string requestJson)
    {
        try
        {
            var client = await GetConnectedClientAsync();
            if (client == null)
                return null;

            var stream = client.GetStream();
            var requestBytes = Encoding.UTF8.GetBytes(requestJson + "\n");
            await stream.WriteAsync(requestBytes);

            using var reader = new StreamReader(stream, Encoding.UTF8, leaveOpen: true);
            var responseLine = await reader.ReadLineAsync();
            _loggedConnectFailure = false;
            return responseLine;
        }
        catch (Exception ex)
        {
            _client?.Dispose();
            _client = null;
            if (!_loggedConnectFailure)
            {
                Log.Warn($"[sts_sim_bridge_mod] analyze request failed: {ex.Message}");
                _loggedConnectFailure = true;
            }
            return null;
        }
    }

    private static async Task<TcpClient?> GetConnectedClientAsync()
    {
        if (_client is { Connected: true })
            return _client;

        _client?.Dispose();
        var client = new TcpClient();
        await client.ConnectAsync(Host, Port);
        _client = client;
        return client;
    }
}
