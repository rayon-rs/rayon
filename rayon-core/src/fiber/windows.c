#include <windows.h>

PVOID __rayon_core_get_current_fiber() {
    return GetCurrentFiber();
}
