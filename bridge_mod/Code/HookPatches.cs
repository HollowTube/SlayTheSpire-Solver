using System;
using System.Linq;
using System.Text.Json.Nodes;
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

    // The player's HP at the start of the current fight (round 1), used to
    // derive `actualHpLostSoFar` for the overlay's actual-vs-expected
    // tracking. -1 means "not yet recorded" (first push of this fight).
    private static double _fightStartHp = -1;

    [HarmonyPatch("AfterPlayerTurnStart")]
    [HarmonyPostfix]
    public static void AfterPlayerTurnStart(CombatState combatState, PlayerChoiceContext choiceContext, Player player)
    {
        if (combatState.RoundNumber == 1)
        {
            _fightStartHp = player.Creature.CurrentHp;
            Overlay.ResetFightBaseline();
            PushDeckBaseline(combatState, player);
        }
        PushAnalysis(combatState, player);
    }

    // Fetches this fight's "deck vs. monster, before any cards are drawn"
    // baseline once, in the background, and hands it to the overlay when it
    // resolves. No-op if this fight isn't a single-monster encounter
    // StateBuilder.BuildDeckBaselineRequest covers.
    private static void PushDeckBaseline(CombatState combatState, Player player)
    {
        string? requestJson;
        try
        {
            requestJson = StateBuilder.BuildDeckBaselineRequest(combatState, player);
        }
        catch (Exception ex)
        {
            Log.Warn($"[sts_sim_bridge_mod] Failed to build deck_baseline request: {ex.Message}");
            return;
        }

        if (requestJson == null)
        {
            Overlay.SetBaselineNotAvailable();
            return;
        }

        _ = Task.Run(async () =>
        {
            var response = await AnalysisClient.SendAnalyzeRequestAsync(requestJson);
            if (response == null)
                return;

            Log.Warn($"[sts_sim_bridge_mod] deck_baseline response: {response}");
            try
            {
                var root = JsonNode.Parse(response) as JsonObject;
                var meanHpLost = root?["mean_hp_lost"]?.GetValue<double>();
                if (meanHpLost.HasValue)
                    Overlay.SetDeckBaseline(meanHpLost.Value);
            }
            catch (Exception ex)
            {
                Log.Warn($"[sts_sim_bridge_mod] Failed to parse deck_baseline response: {ex.Message}");
            }
        });
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

        // Best-effort fallback if the round-1 AfterPlayerTurnStart push was
        // missed (e.g. mod loaded mid-fight): treat the first push we see as
        // the fight's starting HP.
        if (_fightStartHp < 0)
            _fightStartHp = player.Creature.CurrentHp;
        var actualHpLostSoFar = _fightStartHp - player.Creature.CurrentHp;

        var (unknownMonsters, unknownCards) = StateBuilder.FindUnsupported(combatState, player);
        Overlay.SetWarnings(unknownMonsters, unknownCards);

        Overlay.SetCalculating(true);
        _ = Task.Run(async () =>
        {
            try
            {
                var response = await AnalysisClient.SendAnalyzeRequestAsync(requestJson);
                if (response != null)
                {
                    Overlay.SetServerStatus(true);
                    Log.Warn($"[sts_sim_bridge_mod] analyze response: {response}");
                    Overlay.UpdateValues(response, actualHpLostSoFar, player.Creature.CurrentHp, player.Creature.MaxHp);
                }
                else
                {
                    Overlay.SetServerStatus(false, AnalysisClient.LastError);
                }
            }
            finally
            {
                Overlay.SetCalculating(false);
                _requestInFlight = false;
            }
        });
    }
}
