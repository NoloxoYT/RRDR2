# ABI RRDR.Bridge (`rrdr-bridge`) v3

L'ABI est volontairement petite et stable. Ajouter une native ne change pas
l'ABI : tout passe par `call_native`.

Toutes les fonctions sont `extern "C"`, en x86_64. Le shim C# utilise
`CallingConvention.Cdecl` (identique à l'ABI Microsoft x64 en pratique).

## Symboles exportés par le cdylib Rust

```c
uint32_t rd_abi_version(void);                 // 3
int32_t  rd_init(const RdHost *host, const char *config_path);      // v2 fallback
int32_t  rd_init_v3(const RdHostV3 *host, const char *config_path); // v3
void     rd_tick(float delta_seconds);
void     rd_shutdown(void);
int32_t  rd_reload_config(const char *config_path);
```

Le shim C# appelle `rd_init_v3` quand `rd_abi_version()` vaut 3, et `rd_init`
pour un core v2. `rd_init` ignore le provider de pools (les pools globaux
renvoient `None`).

Codes retour de `rd_init` / `rd_reload_config` :

Les codes sont **négatifs** (côté Rust, `ffi.rs`). Un loader ne doit comparer
que `status != 0` : ne jamais tester un code précis côté C++/C# sans avoir mis
à jour cette table en même temps que `rrdr-bridge/src/ffi.rs`. (Les versions
antérieures de `RRDR.Asi.cpp` utilisaient des valeurs positives ; le chargement
fonctionnait parce que seul `!= 0` était comparé.)

| Code | Signification |
|-----:|---------------|
| 0 | OK |
| -1 | `host` NULL |
| -2 | `host->call_native` NULL |
| -3 | `Mod::init` a renvoyé une erreur |
| -4 | panic rattrapé dans l'ABI |
| -5 | `rd_reload_config` appelé sans état initialisé |

## Structures

```c
typedef struct {
    void *user;
    uint64_t (*call_native)(void *user, uint64_t hash,
                            const RdArg *args, uint32_t argc,
                            uint32_t return_kind, uint8_t *return_buffer,
                            uint32_t return_capacity);
    void (*log)(void *user, uint32_t level, const uint8_t *message, uint32_t len);
} RdHost;

typedef struct {
    void *user;
    CallNativeFn call_native;
    LogFn log;
    // Optionnel : fournit worldGetAllPeds / worldGetAllVehicles / worldGetAllObjects.
    uint32_t (*pool_handles)(void *user, uint32_t kind,
                             int32_t *output, uint32_t capacity);
} RdHostV3;

typedef struct {
    uint32_t kind;   // type de l'argument
    uint32_t pad;    // 0
    uint64_t bits;   // valeur ou pointeur
} RdArg;
```

Taille de `RdArg` : 16 octets. `RdHost` : 3 pointeurs (24 octets).
`RdHostV3` : 4 pointeurs (32 octets).

### Types d'arguments (`RdArg.kind`)

| Valeur | Constante | `bits` contient |
|-------:|-----------|-----------------|
| 0 | `ARG_I32` | `int32_t` dans les 32 bits bas |
| 1 | `ARG_U32` | `uint32_t` |
| 2 | `ARG_F32` | bits IEEE-754 de `float` |
| 3 | `ARG_BOOL` | 0 ou 1 |
| 4 | `ARG_U64` | `uint64_t` (handle, hash, ...) |
| 5 | `ARG_STR` | pointeur UTF-8 NUL-terminé, valide pendant l'appel |
| 6 | `ARG_PTR` | adresse 64 bits d'un paramètre de sortie |

### Pools globaux (`pool_handles`)

`pool_handles` est optionnel. Le callback écrit au plus `capacity` handles
`int32_t` dans `output` et renvoie le nombre écrit.

| `kind` | Constante | Source |
|-------:|-----------|--------|
| 0 | `PoolKind::Ped` | `ScriptHookRDR2.dll!?worldGetAllPeds@@YAHPEAHH@Z` |
| 1 | `PoolKind::Vehicle` | `?worldGetAllVehicles@@YAHPEAHH@Z` |
| 2 | `PoolKind::Object` | `?worldGetAllObjects@@YAHPEAHH@Z` |

Les handles retournés sont bruts : `rrdr-game` les valide avec
`DOES_ENTITY_EXIST` / `GET_ENTITY_TYPE` avant de les exposer.

### Types de retour (`return_kind`)

| Valeur | Constante | Comportement |
|-------:|-----------|--------------|
| 0 | `RET_U64` | le shim renvoie le contenu brut du slot 64 bits |
| 1 | `RET_VEC3` | le shim écrit 3 `float` (12 octets) dans `return_buffer` |
| 2 | `RET_STRING` | le shim écrit l'UTF-8 dans `return_buffer` et renvoie la longueur |

Pour `RET_VEC3`, `return_buffer` doit pointer vers au moins 12 octets valides.
Le shim appelle `Function.Call<Vector3>` puis copie `X, Y, Z`.

Pour `RET_STRING`, `return_capacity` donne la taille du buffer. Le shim appelle
`Function.Call<string>`, encode en UTF-8 et copie au plus `return_capacity`
octets ; la valeur de retour est le nombre d'octets écrits. L'implémentation
Rust fournit un buffer de 4 Kio, donc les chaînes plus longues sont tronquées.

## Niveaux de log

| Valeur | Niveau |
|-------:|--------|
| 0 | Error |
| 1 | Warn |
| 2 | Info |
| 3 | Debug |

## Règles

- `rd_tick` est appelé sur le thread du script. Tous les callbacks
  `call_native`/`log` sont synchrones sur ce thread.
- Ne pas appeler `rd_tick` en parallèle : le shim ne le fait pas.
- Les chaînes (`ARG_STR`) ne doivent pas être conservées après le retour de
  `call_native`.
- Les pointeurs de sortie doivent rester valides jusqu'au retour de
  `call_native` (une variable locale sur la pile suffit).
- Une panic Rust est rattrapée dans `rd_init`/`rd_tick`/`rd_shutdown` et
  journalisée ; elle ne doit pas traverser la frontière FFI.
