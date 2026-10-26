RRDR - RustRDR
==============

RDR2 mods written in Rust, loaded by ScriptHookRDR2 (native ASI) or
ScriptHookRDR2DotNet (managed host).

INSTALLATION
------------

Copie dans le dossier du jeu (la ou se trouve RDR2.exe) :

    RRDR.asi
    RRDR.asi.ini

Puis copie le dossier scripts/ :

    scripts/Rscripts/*
    scripts/RRDR.Host.dll        (optionnel, seulement pour ScriptHookRDR2DotNet)
    scripts/RRDR.Host.ini        (optionnel)

Arborescence finale :

    Red Dead Redemption 2\
      RRDR.asi
      RRDR.asi.ini
      dinput8.dll                (ASI loader)
      ScriptHookRDR2.dll
      scripts\
        Rscripts\
          RealisticDamagesCore.dll
          RealisticDamages.ini
          better_economy.dll
          ...

Le loader RRDR.asi scanne scripts\Rscripts et charge chaque DLL qui exporte
rd_abi_version. Ajouter un mod = ajouter un DLL dans ce dossier.

CHOISIR UN LOADER
-----------------

Deux loaders font le meme travail. Ne les utilise pas en meme temps, sinon
chaque mod tourne deux fois par frame.

  1. RRDR.asi (recommande)
     - natif, pas de .NET, pas de ScriptHookRDR2DotNet
     - necessite ScriptHookRDR2.dll et dinput8.dll
     - log : scripts\Rscripts\RRDR.asi.log

  2. RRDR.Host.dll (managed)
     - necessite ScriptHookRDR2DotNet V2 et .NET Framework 4.8
     - place-le dans scripts\Rscripts avec RRDR.Host.ini
     - log : scripts\Rscripts\RRDR.Host.log

CONFIGURATION
-------------

RRDR.asi.ini (dossier du jeu) :

    mods_dir =            dossier des cores, vide = scripts\Rscripts
    log_file =            log du loader
    reload_config =       recharger les .ini des cores a chaud
    disable =             liste de DLL a ignorer, separee par des virgules

Chaque core lit son propre fichier, par exemple RealisticDamages.ini.
Exemple de RRDR.Host.ini pour le loader managed :

    mods_dir =
    log_file = RRDR.Host.log
    reload_config = true
    disable =

LOGS
----

    scripts\Rscripts\RRDR.asi.log     (loader natif)
    scripts\Rscripts\RRDR.Host.log    (loader managed)

DESINSTALLATION
---------------

Supprime RRDR.asi, RRDR.asi.ini et le dossier scripts\Rscripts.
Si le jeu a plante sans appeler rd_shutdown, redemarre le jeu pour remettre
l'etat global a zero.
