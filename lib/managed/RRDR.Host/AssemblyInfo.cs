// Test-shim visibility only. The shipping net48 real-API build does not define
// MOCK_API, so the internals of RRDR.Host are not exposed in the game DLL.
#if MOCK_API
using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("RRDR.Sim")]
#endif
