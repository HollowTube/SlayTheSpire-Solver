using System;
using System.Linq;
using System.Threading.Tasks;
using HarmonyLib;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Hooks;
using MegaCrit.Sts2.Core.Logging;

namespace sts_sim_bridge_mod;

/// <summary>
/// Postfix-patches the Hook class's static dispatchers so every player-turn
/// start / card play pushes the current combat state to sts_sim.server and
/// logs back the move values.
/// </summary>
[HarmonyPatch(typeof(Hook))]
public static class HookPatches
{
    // Avoids piling up requests if a card is played again before the
    // previous push's response has logged.
    private static volatile bool _requestInFlight;

    [HarmonyPatch("AfterPlayerTurnStart")]
    [HarmonyPostfix]
    public static void AfterPlayerTurnStart(CombatState combatState, PlayerChoiceContext choiceContext, Player player)
    {
        PushAnalysis(combatState, player);
    }

    [HarmonyPatch("AfterCardPlayed")]
    [HarmonyPostfix]
    public static void AfterCardPlayed(CombatState combatState, PlayerChoiceContext choiceContext, CardPlay cardPlay)
    {
        var player = combatState.Players.FirstOrDefault();
        if (player != null)
            PushAnalysis(combatState, player);
    }

    private static void PushAnalysis(CombatState combatState, Player player)
    {
        if (_requestInFlight)
            return;
        _requestInFlight = true;

        string requestJson;
        try
        {
            requestJson = StateBuilder.BuildAnalyzeRequest(combatState, player);
        }
        catch (Exception ex)
        {
            Log.Warn($"[sts_sim_bridge_mod] Failed to build analyze request: {ex.Message}");
            _requestInFlight = false;
            return;
        }

        _ = Task.Run(async () =>
        {
            try
            {
                var response = await AnalysisClient.SendAnalyzeRequestAsync(requestJson);
                if (response != null)
                    Log.Warn($"[sts_sim_bridge_mod] analyze response: {response}");
            }
            finally
            {
                _requestInFlight = false;
            }
        });
    }
}
