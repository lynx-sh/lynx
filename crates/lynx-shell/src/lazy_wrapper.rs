// This module is intentionally thin — wrapper generation lives in lynx-plugin::lazy.
// lynx-shell re-exports it so the init code can call it without a direct dep on lynx-plugin.
// The actual shell integration is: lx init emits generate_lazy_wrappers() output for
// each lazy plugin in the resolved load order.
//
// No logic here — see lynx-plugin::lazy for the implementation.
