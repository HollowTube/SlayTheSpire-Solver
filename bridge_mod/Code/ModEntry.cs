using HarmonyLib;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;

namespace sts_sim_bridge_mod;

[ModInitializer("Init")]
public static class ModEntry
{
    private static Harmony? _harmony;

    public static void Init()
    {
        Log.Warn("[sts_sim_bridge_mod] Initializing...");

        _harmony = new Harmony("com.tritintruong.stssimbridgemod");
        _harmony.PatchAll();

        Log.Warn("[sts_sim_bridge_mod] Loaded successfully!");
    }
}
