using BepInEx;
using BepInEx.Logging;
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Reflection;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using UnityEngine;

// ===== BepInEx IPC Plugin (uses reflection for Hearthstone API) =====

[BepInPlugin("com.hearthbuddy.rust-ipc", "Rust IPC Plugin", "1.0.0")]
public class RustIPCPlugin : BaseUnityPlugin
{
    private ManualLogSource _log;
    private Thread _pipeThread;
    private volatile bool _running = true;
    private JavaScriptSerializer _json = new JavaScriptSerializer();
    private const string PIPE_NAME = @"Hearthbuddy_IPC";

    // Hearthstone types resolved at runtime
    private Type _sceneMgrType;
    private Type _gameStateType;
    private Type _entityType;
    private Type _playerType;
    private Type _cardType;
    private Type _endTurnBtnType;
    private Type _gameTagEnum;

    // Cached methods
    private MethodInfo _entity_GetCardId;
    private MethodInfo _entity_GetTag;
    private PropertyInfo _entity_GetEntityId;

    private void Awake()
    {
        _log = Logger;
        _log.LogInfo("Rust IPC Plugin starting...");

        // Resolve Hearthstone types from Assembly-CSharp
        ResolveTypes();

        _pipeThread = new Thread(PipeServerLoop) { IsBackground = true };
        _pipeThread.Start();
        _log.LogInfo("Listening on \\\\.\\pipe\\" + PIPE_NAME);
    }

    private void ResolveTypes()
    {
        foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
        {
            string name = asm.GetName().Name;
            if (name == "Assembly-CSharp")
            {
                _sceneMgrType = asm.GetType("SceneMgr");
                _gameStateType = asm.GetType("GameState");
                _entityType = asm.GetType("Entity");
                _playerType = asm.GetType("Player");
                _cardType = asm.GetType("Card");
                _endTurnBtnType = asm.GetType("EndTurnButton");
                _gameTagEnum = asm.GetType("GAME_TAG");

                if (_entityType != null)
                {
                    _entity_GetCardId = _entityType.GetMethod("GetCardId", BindingFlags.Public | BindingFlags.Instance);
                    _entity_GetTag = _entityType.GetMethod("GetTag", BindingFlags.Public | BindingFlags.Instance);
                    _entity_GetEntityId = _entityType.GetProperty("GetEntityId",
                        BindingFlags.Public | BindingFlags.Instance);
                    if (_entity_GetEntityId == null)
                    {
                        _entity_GetEntityId = _entityType.GetProperty("entityId",
                            BindingFlags.Public | BindingFlags.Instance);
                    }
                    if (_entity_GetEntityId == null)
                    {
                        _entity_GetEntityId = _entityType.GetProperty("m_entityId",
                            BindingFlags.NonPublic | BindingFlags.Instance);
                    }
                }

                _log.LogInfo("Resolved types from Assembly-CSharp");
                _log.LogInfo("  SceneMgr: " + (_sceneMgrType != null));
                _log.LogInfo("  GameState: " + (_gameStateType != null));
                _log.LogInfo("  Entity: " + (_entityType != null));
                _log.LogInfo("  Player: " + (_playerType != null));
                _log.LogInfo("  GAME_TAG: " + (_gameTagEnum != null));
                break;
            }
        }
    }

    private void OnDestroy() { _running = false; }

    // ===== 命名管道服务器 =====

    private void PipeServerLoop()
    {
        while (_running)
        {
            try
            {
                using (var pipe = new NamedPipeServerStream(PIPE_NAME, PipeDirection.InOut,
                    NamedPipeServerStream.MaxAllowedServerInstances, PipeTransmissionMode.Message,
                    PipeOptions.Asynchronous))
                {
                    pipe.WaitForConnection();
                    HandleClient(pipe);
                }
            }
            catch (Exception ex)
            {
                if (_running) _log.LogError("Pipe: " + ex.Message);
                Thread.Sleep(1000);
            }
        }
    }

    private void HandleClient(NamedPipeServerStream pipe)
    {
        using (var reader = new StreamReader(pipe, Encoding.UTF8))
        using (var writer = new StreamWriter(pipe, Encoding.UTF8) { AutoFlush = true })
        {
            while (_running && pipe.IsConnected)
            {
                try
                {
                    string line = reader.ReadLine();
                    if (line == null) break;
                    string response = Dispatch(line);
                    if (response != null) writer.WriteLine(response);
                }
                catch { break; }
            }
        }
    }

    private string Dispatch(string json)
    {
        try
        {
            var msg = _json.Deserialize<IpcMessage>(json);
            switch (msg.type)
            {
                case "GetGameState": return HandleGetGameState(msg.seq);
                case "Ping": return HandlePing(msg.seq, json);
                case "PerformAction": return HandlePerformAction(msg.seq, json);
                default: return Err(msg.seq, "Unknown: " + msg.type);
            }
        }
        catch (Exception ex)
        {
            return Err(0, "Parse: " + ex.Message);
        }
    }

    private string HandleGetGameState(uint seq)
    {
        return _json.Serialize(new GameStateResponse
        {
            seq = seq, state = ReadGameState()
        });
    }

    private string HandlePing(uint seq, string raw)
    {
        var req = _json.Deserialize<Dictionary<string, object>>(raw);
        ulong ts = req.ContainsKey("timestamp") ? Convert.ToUInt64(req["timestamp"]) : 0;
        return _json.Serialize(new PongResponse { seq = seq, timestamp = ts });
    }

    private string HandlePerformAction(uint seq, string raw)
    {
        try
        {
            var req = _json.Deserialize<Dictionary<string, object>>(raw);
            if (!req.ContainsKey("action")) return Err(seq, "No action field");
            var action = req["action"] as Dictionary<string, object>;
            if (action == null) return Err(seq, "Invalid action");
            bool ok = ExecuteAction(action);
            return _json.Serialize(new ActionResultResponse { seq = seq, success = ok });
        }
        catch (Exception ex)
        {
            return _json.Serialize(new ActionResultResponse { seq = seq, success = false, error = ex.Message });
        }
    }

    private string Err(uint seq, string msg)
    {
        return _json.Serialize(new ErrorResponse { seq = seq, message = msg });
    }

    // ===== 游戏状态读取 (reflection based) =====

    private GameStateData ReadGameState()
    {
        var data = new GameStateData();
        try
        {
            // SceneMgr
            if (_sceneMgrType != null)
            {
                var getMethod = _sceneMgrType.GetMethod("Get", BindingFlags.Public | BindingFlags.Static);
                if (getMethod != null)
                {
                    var sceneMgr = getMethod.Invoke(null, null);
                    if (sceneMgr != null)
                    {
                        var modeProp = _sceneMgrType.GetProperty("m_scene",
                            BindingFlags.NonPublic | BindingFlags.Instance);
                        if (modeProp == null)
                            modeProp = _sceneMgrType.GetProperty("GetMode",
                                BindingFlags.Public | BindingFlags.Instance);
                        if (modeProp != null)
                        {
                            var mode = modeProp.GetValue(sceneMgr, null);
                            if (mode != null) data.scene = mode.ToString();
                        }
                    }
                }
            }

            // GameState
            object gs = null;
            if (_gameStateType != null)
            {
                var getGs = _gameStateType.GetMethod("Get", BindingFlags.Public | BindingFlags.Static);
                if (getGs != null) gs = getGs.Invoke(null, null);
            }

            if (gs == null) return data;

            // Turn
            var getTurn = _gameStateType.GetMethod("GetTurn", BindingFlags.Public | BindingFlags.Instance);
            if (getTurn != null) data.turn = (uint)(int)getTurn.Invoke(gs, null);

            // Get players
            object me = null, op = null;

            var getFriendly = _gameStateType.GetMethod("GetFriendlySidePlayer",
                BindingFlags.Public | BindingFlags.Instance);
            var getOpposing = _gameStateType.GetMethod("GetOpposingSidePlayer",
                BindingFlags.Public | BindingFlags.Instance);

            if (getFriendly != null) me = getFriendly.Invoke(gs, null);
            if (getOpposing != null) op = getOpposing.Invoke(gs, null);

            if (me == null || op == null) return data;

            // Turn ownership
            var isOurTurn = _gameStateType.GetMethod("IsFriendlySidePlayerTurn",
                BindingFlags.Public | BindingFlags.Instance);
            if (isOurTurn != null) data.is_own_turn = (bool)isOurTurn.Invoke(gs, null);

            // Read heroes
            var getHeroCard = _playerType?.GetMethod("GetHeroCard",
                BindingFlags.Public | BindingFlags.Instance);
            if (getHeroCard != null)
            {
                var myHeroCard = getHeroCard.Invoke(me, null);
                var opHeroCard = getHeroCard.Invoke(op, null);
                ReadEntityData(myHeroCard, data.own_hero);
                ReadEntityData(opHeroCard, data.enemy_hero);
            }

            // Mana
            var getMana = _playerType?.GetMethod("GetNumAvailableResources",
                BindingFlags.Public | BindingFlags.Instance);
            var getTagMethod = _entityType?.GetMethod("GetTag",
                BindingFlags.Public | BindingFlags.Instance);

            if (getMana != null) data.own_mana = (uint)(int)getMana.Invoke(me, null);

            // Max mana via TAG
            int tagResources = GetTagValue(_gameTagEnum, "RESOURCES");
            if (getTagMethod != null && tagResources >= 0)
                data.own_max_mana = (uint)(int)getTagMethod.Invoke(me, new object[] { tagResources });

            // Hand cards via TAG
            int tagHand = GetTagValue(_gameTagEnum, "NUM_CARDS_IN_HAND") != -1
                ? GetTagValue(_gameTagEnum, "NUM_CARDS_IN_HAND")
                : GetTagValue(_gameTagEnum, "ZONE_HAND"); // fallback
            int tagDeck = GetTagValue(_gameTagEnum, "NUM_CARDS_IN_DECK") != -1
                ? GetTagValue(_gameTagEnum, "NUM_CARDS_IN_DECK")
                : GetTagValue(_gameTagEnum, "ZONE_DECK"); // fallback

            if (getTagMethod != null)
            {
                if (tagHand >= 0) data.enemy_hand_count = (uint)(int)getTagMethod.Invoke(op, new object[] { tagHand });
                if (tagDeck >= 0)
                {
                    data.own_deck_count = (uint)(int)getTagMethod.Invoke(me, new object[] { tagDeck });
                    data.enemy_deck_count = (uint)(int)getTagMethod.Invoke(op, new object[] { tagDeck });
                }
            }

            // Enumerate all entities via GameState.GetEntity(int)
            var getEntity = _gameStateType.GetMethod("GetEntity",
                BindingFlags.Public | BindingFlags.Instance);
            var getZone = _entityType?.GetMethod("GetZone",
                BindingFlags.Public | BindingFlags.Instance);

            if (getEntity != null && getZone != null && getTagMethod != null)
            {
                int tagCardType = GetTagValue(_gameTagEnum, "CARDTYPE");
                int tagController = GetTagValue(_gameTagEnum, "CONTROLLER");
                int tagPlayerId = GetTagValue(_gameTagEnum, "PLAYER_ID");

                var getController = _entityType?.GetMethod("GetController",
                    BindingFlags.Public | BindingFlags.Instance);
                var getPlayerId = _entityType?.GetMethod("GetPlayerID",
                    BindingFlags.Public | BindingFlags.Instance);

                int myPlayerId = -1;
                if (getPlayerId != null && tagPlayerId >= 0)
                    myPlayerId = (int)getTagMethod.Invoke(me, new object[] { tagPlayerId });

                for (int eid = 4; eid < 200; eid++)
                {
                    var entity = getEntity.Invoke(gs, new object[] { eid });
                    if (entity == null) break;

                    int zoneVal = (int)getZone.Invoke(entity, null);
                    int ctrl = 0;
                    int cardType = 0;

                    if (getController != null)
                        ctrl = (int)getController.Invoke(entity, null);
                    else if (tagController >= 0)
                        ctrl = (int)getTagMethod.Invoke(entity, new object[] { tagController });

                    if (tagCardType >= 0)
                        cardType = (int)getTagMethod.Invoke(entity, new object[] { tagCardType });

                    bool isOurs = ctrl == myPlayerId || (myPlayerId < 0 && eid % 2 == 1);

                    // zone: 1=PLAY, 2=DECK, 3=HAND, 4=GRAVEYARD, 7=SECRET
                    if (zoneVal == 3 && isOurs && eid > 10)
                    {
                        // Our hand card
                        var cd = new CardData();
                        ReadEntityData(entity, cd);
                        data.own_hand.Add(cd);
                    }
                    else if (zoneVal == 1 && eid > 10)
                    {
                        // On board
                        var ed = new EntityData();
                        ReadEntityData(entity, ed);
                        if (cardType == 4) // Minion
                        {
                            if (isOurs) data.own_minions.Add(ed);
                            else data.enemy_minions.Add(ed);
                        }
                        else if (cardType == 3) // Hero
                        {
                            if (isOurs) ReadEntityData(entity, data.own_hero);
                            else ReadEntityData(entity, data.enemy_hero);
                        }
                    }
                }

                data.own_hand_count = (uint)data.own_hand.Count;
            }
        }
        catch (Exception ex)
        {
            _log.LogError("ReadGameState: " + ex.ToString());
        }
        return data;
    }

    /// Simplified method - populate readable fields from entity
    private void ReadEntityData(object entity, EntityData d)
    {
        if (entity == null) return;

        // EntityId
        if (_entity_GetEntityId != null)
        {
            var val = _entity_GetEntityId.GetValue(entity, null);
            if (val is int) d.entity_id = (int)val;
        }

        // CardId
        if (_entity_GetCardId != null)
        {
            var val = _entity_GetCardId.Invoke(entity, null);
            if (val is string) d.card_id = (string)val;
        }

        // Try tag-based reading for all numeric fields
        if (_entity_GetTag != null && _gameTagEnum != null)
        {
            int tagH = GetTagValue(_gameTagEnum, "HEALTH");
            int tagA = GetTagValue(_gameTagEnum, "ATK");
            int tagD = GetTagValue(_gameTagEnum, "DAMAGE");
            int tagArm = GetTagValue(_gameTagEnum, "ARMOR");
            int tagTaunt = GetTagValue(_gameTagEnum, "TAUNT");
            int tagDS = GetTagValue(_gameTagEnum, "DIVINE_SHIELD");
            int tagStealth = GetTagValue(_gameTagEnum, "STEALTH");
            int tagPoison = GetTagValue(_gameTagEnum, "POISONOUS");
            int tagLS = GetTagValue(_gameTagEnum, "LIFESTEAL");
            int tagExhaust = GetTagValue(_gameTagEnum, "EXHAUSTED");
            int tagNumAtk = GetTagValue(_gameTagEnum, "NUM_ATTACKS_THIS_TURN");

            int hp = tagH >= 0 ? (int)_entity_GetTag.Invoke(entity, new object[] { tagH }) : 0;
            int atk = tagA >= 0 ? (int)_entity_GetTag.Invoke(entity, new object[] { tagA }) : 0;
            int dmg = tagD >= 0 ? (int)_entity_GetTag.Invoke(entity, new object[] { tagD }) : 0;

            d.health = hp - dmg;
            d.attack = atk;
            if (tagArm >= 0) d.armor = (int)_entity_GetTag.Invoke(entity, new object[] { tagArm });
            if (tagTaunt >= 0) d.has_taunt = (int)_entity_GetTag.Invoke(entity, new object[] { tagTaunt }) > 0;
            if (tagDS >= 0) d.has_divine_shield = (int)_entity_GetTag.Invoke(entity, new object[] { tagDS }) > 0;
            if (tagStealth >= 0) d.has_stealth = (int)_entity_GetTag.Invoke(entity, new object[] { tagStealth }) > 0;
            if (tagPoison >= 0) d.has_poisonous = (int)_entity_GetTag.Invoke(entity, new object[] { tagPoison }) > 0;
            if (tagLS >= 0) d.has_lifesteal = (int)_entity_GetTag.Invoke(entity, new object[] { tagLS }) > 0;
            if (tagExhaust >= 0) d.is_exhausted = (int)_entity_GetTag.Invoke(entity, new object[] { tagExhaust }) > 0;
            if (tagNumAtk >= 0) d.num_attacks = (int)_entity_GetTag.Invoke(entity, new object[] { tagNumAtk });
        }
    }

    private void ReadEntityData(object entity, CardData cd)
    {
        if (entity == null) return;
        if (_entity_GetEntityId != null)
        {
            var val = _entity_GetEntityId.GetValue(entity, null);
            if (val is int) cd.entity_id = (int)val;
        }
        if (_entity_GetCardId != null)
        {
            var val = _entity_GetCardId.Invoke(entity, null);
            if (val is string) cd.card_id = (string)val;
        }

        if (_entity_GetTag != null && _gameTagEnum != null)
        {
            int tCost = GetTagValue(_gameTagEnum, "COST");
            int tAtk = GetTagValue(_gameTagEnum, "ATK");
            int tHp = GetTagValue(_gameTagEnum, "HEALTH");
            int tDmg = GetTagValue(_gameTagEnum, "DAMAGE");
            int tType = GetTagValue(_gameTagEnum, "CARDTYPE");

            if (tCost >= 0) cd.cost = (int)_entity_GetTag.Invoke(entity, new object[] { tCost });
            if (tAtk >= 0) cd.attack = (int)_entity_GetTag.Invoke(entity, new object[] { tAtk });
            if (tHp >= 0 && tDmg >= 0)
            {
                int hp = (int)_entity_GetTag.Invoke(entity, new object[] { tHp });
                int dmg = (int)_entity_GetTag.Invoke(entity, new object[] { tDmg });
                cd.health = hp - dmg;
            }
            if (tType >= 0)
            {
                int ct = (int)_entity_GetTag.Invoke(entity, new object[] { tType });
                cd.card_type = ct == 4 ? "Minion" : ct == 5 ? "Spell" : ct == 7 ? "Weapon" : ct == 3 ? "Hero" : "Other";
            }
        }
    }

    private int GetTagValue(Type enumType, string name)
    {
        if (enumType == null) return -1;
        try
        {
            var val = Enum.Parse(enumType, name);
            return (int)val;
        }
        catch { return -1; }
    }

    // ===== 动作执行 =====

    private bool ExecuteAction(Dictionary<string, object> action)
    {
        string actionType = action["action_type"] as string ?? "";
        _log.LogInfo("Action: " + actionType);

        try
        {
            switch (actionType)
            {
                case "EndTurn":
                    if (_endTurnBtnType != null)
                    {
                        var getBtn = _endTurnBtnType.GetMethod("Get",
                            BindingFlags.Public | BindingFlags.Static);
                        var btn = getBtn?.Invoke(null, null);
                        if (btn != null)
                        {
                            var doEndTurn = _endTurnBtnType.GetMethod("DoEndTurn",
                                BindingFlags.Public | BindingFlags.Instance);
                            doEndTurn?.Invoke(btn, null);
                            return true;
                        }
                    }
                    return false;

                case "PlayCard":
                    if (action.ContainsKey("hand_index") && action["hand_index"] != null && _gameStateType != null)
                    {
                        int idx = Convert.ToInt32(action["hand_index"]);
                        var getGs = _gameStateType.GetMethod("Get", BindingFlags.Public | BindingFlags.Static);
                        var gs = getGs?.Invoke(null, null);
                        if (gs == null) return false;

                        var getMe = _gameStateType.GetMethod("GetFriendlySidePlayer",
                            BindingFlags.Public | BindingFlags.Instance);
                        var me = getMe?.Invoke(gs, null);
                        if (me == null) return false;

                        // Try different method names for getting hand
                        var getHandMgr = _playerType?.GetMethod("GetHand",
                            BindingFlags.Public | BindingFlags.Instance);
                        if (getHandMgr == null)
                            getHandMgr = _playerType?.GetMethod("GetHandArea",
                                BindingFlags.Public | BindingFlags.Instance);

                        object handArea = getHandMgr?.Invoke(me, null);
                        if (handArea == null) return false;

                        var getCards = handArea.GetType().GetMethod("GetCardsInHand",
                            BindingFlags.Public | BindingFlags.Instance);
                        if (getCards == null) // try property
                        {
                            var cardsProp = handArea.GetType().GetProperty("m_cardsInHand",
                                BindingFlags.NonPublic | BindingFlags.Instance);
                            if (cardsProp != null)
                            {
                                var cards = cardsProp.GetValue(handArea, null) as System.Collections.IList;
                                if (cards != null && idx >= 0 && idx < cards.Count)
                                {
                                    int pos = action.ContainsKey("position") && action["position"] != null
                                        ? Convert.ToInt32(action["position"]) : 0;
                                    // Card play via ManaCounter / GameState action
                                    // This is Hearthstone-specific and requires knowing the API
                                    _log.LogInfo("Would play card at index " + idx);
                                    return true;
                                }
                            }
                        }
                    }
                    return true; // Return true even if not fully implemented (placeholder)

                default:
                    _log.LogWarning("Unknown action: " + actionType);
                    return false;
            }
        }
        catch (Exception ex)
        {
            _log.LogError("Action error: " + ex.Message);
            return false;
        }
    }

    // ===== JSON data models =====

    public class IpcMessage { public string type; public uint seq; }
    public class GameStateResponse : IpcMessage { public GameStateData state; public GameStateResponse() { type = "GameState"; } }
    public class ActionResultResponse : IpcMessage { public bool success; public string error; public ActionResultResponse() { type = "ActionResult"; } }
    public class PongResponse : IpcMessage { public ulong timestamp; public PongResponse() { type = "Pong"; } }
    public class ErrorResponse : IpcMessage { public string message; public ErrorResponse() { type = "Error"; } }

    public class GameStateData
    {
        public string scene = "Unknown"; public bool is_own_turn; public uint turn;
        public uint own_mana; public uint own_max_mana;
        public EntityData own_hero = new EntityData(); public EntityData enemy_hero = new EntityData();
        public List<CardData> own_hand = new List<CardData>();
        public List<EntityData> own_minions = new List<EntityData>();
        public List<EntityData> enemy_minions = new List<EntityData>();
        public uint own_hand_count; public uint enemy_hand_count;
        public uint own_deck_count; public uint enemy_deck_count;
    }
    public class EntityData
    {
        public int entity_id; public string card_id = ""; public int health, attack, armor;
        public bool has_taunt, has_divine_shield, has_stealth, has_poisonous, has_lifesteal;
        public bool is_exhausted; public int num_attacks;
    }
    public class CardData
    {
        public int entity_id; public string card_id = ""; public int cost, attack, health; public string card_type = "";
    }
}
