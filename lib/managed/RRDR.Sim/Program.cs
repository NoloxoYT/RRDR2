using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using RDR2.Native;
using RRDR.Host;

namespace RRDR.Sim
{
    /// <summary>
    /// End-to-end test of the real managed host + real Rust cdylib, using the
    /// mock ScriptHook assembly and a fake native dispatcher. This is the
    /// "run it without the game" path.
    /// </summary>
    internal static class Program
    {
        private const ulong PlayerId = 0x217E9DC48139933D;
        private const ulong GetPlayerPed = 0x275F255ED201B937;
        private const ulong DoesEntityExist = 0xD42BD6EB2E0F1677;
        private const ulong GetEntityType = 0x97F696ACA466B4E0;
        private const ulong IsEntityDead = 0x7D5B1F88E7504BBA;
        private const ulong GetEntityHealth = 0x82368787EA73C0F7;
        private const ulong GetPedMaxHealth = 0x4700A416E8324EF3;
        private const ulong GetPedLastDamageBone = 0xD75960F6BD9EA49C;
        private const ulong SetEntityHealth = 0xAC2767ED8BDFAB15;
        private const ulong SetLocalPlayerDamageMultiplier = 0xD041A32992A55F84;
        private const ulong SetAiWeaponDamageModifier = 0x1B1E2A40A65B8521;
        private const ulong SetAiMeleeWeaponDamageModifier = 0x66460DEDDD417254;
        private const ulong ResetAiWeaponDamageModifier = 0xEA16670E7BA4743C;
        private const ulong ClearPedLastDamageBone = 0x8EF6B7AC68E2F01B;
        private const ulong ClearPedLastWeaponDamage = 0x087D8F4BC65F68E4;
        private const ulong IsPedDeadOrDying = 0x3317DEDB88C95038;
        private const ulong GetPedLastWeaponImpactCoord = 0x6C4D0409BA1A2BC2;

        // PLAYER::_GET_PLAYER_HEALTH etc. are not used by the mod, but a few
        // entity helpers are needed for zone logging.
        private const ulong GetEntityCoords = 0xA86D5F069399F44D;
        private const ulong GetPedBoneCoords = 0x17C07FC640E86B4E;
        private const ulong GetPedCurrentWeapon = 0x8425C5F057012DAB;
        private const ulong IsWeaponAGun = 0x705BE297EEBDB95D;
        private const ulong GetWeaponAttachPoint = 0x65DC4AC5B96614CB;
        private const ulong SetAllowAnyWeaponDrop = 0x78030C7867D8B9B6;
        private const ulong MakePedDropWeapon = 0xCEF4C65DE502D367;
        private const ulong GetInventoryIdFromPed = 0x13D234A2A3F66E63;
        private const ulong GetItemCountWithItemId = 0xE787F05DFC977BDE;

        private const uint TorsoBone = 14411;
        private const uint HeadBone = 21030;

        private static int Main()
        {
            string baseDirectory = AppContext.BaseDirectory;
            string logPath = Path.Combine(baseDirectory, "RRDR.Host.log");
            string configPath = Path.Combine(baseDirectory, "RealisticDamages.ini");

            string modsDirectory = Path.Combine(baseDirectory, "Rscripts");
            Directory.CreateDirectory(modsDirectory);

            // The host scans a folder and loads every core it finds. Mirror the
            // game layout so the simulator covers the real code path.
            string sourceCore = Path.Combine(baseDirectory, "libRealisticDamagesCore.so");
            string targetCore = Path.Combine(modsDirectory, "libRealisticDamagesCore.so");
            if (File.Exists(sourceCore))
            {
                File.Copy(sourceCore, targetCore, true);
            }

            File.Delete(logPath);
            File.WriteAllText(configPath, SimConfig(), Encoding.UTF8);
            File.WriteAllText(
                Path.Combine(baseDirectory, "RRDR.Host.ini"),
                "mods_dir = Rscripts\nlog_file = RRDR.Host.log\nreload_config = true\n",
                Encoding.UTF8);

            SimGame game = new SimGame();
            MockNativeHost.Handler = game.HandleNative;
            MockNativeHost.Vector3Handler = game.HandleNativeVector3;
            MockNativeHost.StringHandler = game.HandleNativeString;
            MockNativeHost.PoolHandler = game.HandlePool;

            Console.WriteLine("== RRDR.Host end-to-end simulation ==");
            Console.WriteLine("base: " + baseDirectory);

            var script = new RRDR.Host.Main();

            TickFor(script, 1000);
            Console.WriteLine(
                "init: health={0} playerMult={1} aiMult={2} nativeCalls={3}",
                game.Health,
                game.PlayerMultiplier,
                game.AiWeaponMultiplier,
                game.NativeCalls);

            // Start from a full-health player so the mod's baseline matches.
            game.Health = 200;
            game.MaxHealth = 200;
            TickFor(script, 100);

            game.ApplyDamage(20, TorsoBone);
            TickFor(script, 1000);
            Console.WriteLine("after 20 dmg torso + 1s: health={0}", game.Health);

            game.Health = 200;
            game.LastWeaponImpact = true;

            // A hit needs a damage event: give the mod a baseline first,
            // then apply damage inside the same tick.
            TickFor(script, 100);
            game.ApplyDamage(40, HeadBone);
            game.LastWeaponImpact = true;
            TickFor(script, 250);
            Console.WriteLine("after 40 dmg head (weapon impact): health={0}", game.Health);

            script.RaiseAborted();
            Console.WriteLine(
                "shutdown: playerMult={0} aiResetCount={1} unknownNatives={2}",
                game.PlayerMultiplier,
                game.AiResetCount,
                game.UnknownCalls);

            string log = File.Exists(logPath) ? File.ReadAllText(logPath) : "(no log file)";
            Console.WriteLine();
            Console.WriteLine("== log ==");
            Console.WriteLine(log);

            bool ok = game.Health == 0
                && game.UnknownCalls == 0
                && log.Contains("Rust core loaded")
                && log.Contains("Entry point loaded")
                && log.Contains("bleed: +")
                && log.Contains("bleed tick: -");

            if (!ok)
            {
                Console.Error.WriteLine("SIMULATION FAILED");
                return 1;
            }

            Console.WriteLine("SIMULATION OK");
            return 0;
        }

        private static void TickFor(RRDR.Host.Main script, int milliseconds)
        {
            // Deterministic fixed step: 50 ms per simulated tick.
            int ticks = Math.Max(1, milliseconds / 50);
            for (int index = 0; index < ticks; index++)
            {
                script.TickOnce(0.05f);
            }
        }

        private static string SimConfig()
        {
            return string.Join(Environment.NewLine, new[]
            {
                "enabled = true",
                "player_damage_multiplier = 1.35",
                "ai_weapon_damage_multiplier = 1.15",
                "headshot_lethal = true",
                "headshot_threshold = 0.50",
                "bleed_enabled = true",
                "bleed_damage_per_second = 3.0",
                "bleed_duration_seconds = 15",
                "bleed_min_damage = 5",
                "bleed_can_kill = false",
                "debug = true",
                "tick_interval_ms = 100",
                "reapply_interval_seconds = 5",
                string.Empty
            });
        }

        private sealed class SimGame
        {
            public int Health = 100;
            public int MaxHealth = 100;
            public int LastBone;
            public bool BoneValid;
            public float PlayerMultiplier = 1.0f;
            public float AiWeaponMultiplier = 1.0f;
            public float AiMeleeMultiplier = 1.0f;
            public int AiResetCount;
            public int NativeCalls;
            public int UnknownCalls;
            public bool LastWeaponImpact;

            public void ApplyDamage(int amount, uint bone)
            {
                Health = Math.Max(0, Health - amount);
                LastBone = unchecked((int)bone);
                BoneValid = true;
            }

            public void Heal(int amount)
            {
                Health = Math.Min(MaxHealth, Health + amount);
            }

            public int HandlePool(uint kind, int[] buffer)
            {
                // One ped in the fake global pool.
                if (kind == 0 && buffer.Length > 0)
                {
                    buffer[0] = 1;
                    return 1;
                }

                return 0;
            }

            public RDR2.Math.Vector3 HandleNativeVector3(ulong hash, InputArgument[] arguments)
            {
                if (hash == GetPedLastWeaponImpactCoord)
                {
                    return LastWeaponImpact
                        ? new RDR2.Math.Vector3(1.0f, 2.0f, 3.0f)
                        : new RDR2.Math.Vector3(0.0f, 0.0f, 0.0f);
                }

                // Stand-in for ENTITY::GET_ENTITY_COORDS.
                return new RDR2.Math.Vector3(10.0f, 20.0f, 30.0f);
            }

            public string HandleNativeString(ulong hash, InputArgument[] arguments)
            {
                // Stand-in for DEBUG::GET_GAME_VERSION_NAME.
                return "sim-1.0";
            }

            public ulong HandleNative(ulong hash, InputArgument[] arguments)
            {
                NativeCalls++;

                switch (hash)
                {
                    case PlayerId:
                        return 0;
                    case GetPlayerPed:
                        return 1;
                    case DoesEntityExist:
                        return 1;
                    case GetEntityType:
                        return 1;
                    case IsEntityDead:
                        return 0;
                    case GetEntityHealth:
                        return (ulong)Math.Max(0, Health);
                    case GetPedMaxHealth:
                        return (ulong)MaxHealth;
                    case GetPedLastDamageBone:
                        if (arguments.Length >= 2)
                        {
                            IntPtr pointer = new IntPtr(unchecked((long)arguments[1].RawValue));
                            Marshal.WriteInt32(pointer, LastBone);
                        }
                        return BoneValid ? 1UL : 0UL;
                    case SetEntityHealth:
                        if (arguments.Length >= 2)
                        {
                            Health = Math.Max(0, Math.Min(MaxHealth, arguments[1].AsInt32()));
                        }
                        return 0;
                    case SetLocalPlayerDamageMultiplier:
                        if (arguments.Length >= 2)
                        {
                            PlayerMultiplier = arguments[1].AsSingle();
                        }
                        return 0;
                    case SetAiWeaponDamageModifier:
                        if (arguments.Length >= 1)
                        {
                            AiWeaponMultiplier = arguments[0].AsSingle();
                        }
                        return 0;
                    case SetAiMeleeWeaponDamageModifier:
                        if (arguments.Length >= 1)
                        {
                            AiMeleeMultiplier = arguments[0].AsSingle();
                        }
                        return 0;
                    case ResetAiWeaponDamageModifier:
                        AiWeaponMultiplier = 1.0f;
                        AiResetCount++;
                        return 0;
                    case IsPedDeadOrDying:
                        return Health <= 0 ? 1UL : 0UL;
                    case ClearPedLastDamageBone:
                    case ClearPedLastWeaponDamage:
                    case SetAllowAnyWeaponDrop:
                    case MakePedDropWeapon:
                        return 0;
                    case GetPedLastWeaponImpactCoord:
                        // The native writes the Vector3 through an output
                        // pointer and returns a BOOL.
                        return LastWeaponImpact ? 1UL : 0UL;
                    case GetPedCurrentWeapon:
                        return 0;
                    case IsWeaponAGun:
                        return 1;
                    case GetWeaponAttachPoint:
                        return 0;
                    case GetInventoryIdFromPed:
                        return 0;
                    case GetItemCountWithItemId:
                        return 0;
                    case GetPedBoneCoords:
                        return 0;
                    default:
                        UnknownCalls++;
                        return 0;
                }
            }
        }
    }
}
