#!/usr/bin/env python3
"""Ask an X window to close, the way its title-bar close button does.

    scripts/window-close.py <window id>

Sends the ICCCM WM_DELETE_WINDOW client message, so the game exits through
its normal close path. Use this instead of `wmctrl -c` (WSLg has no EWMH
window manager, so wmctrl can't list or close windows) or `xdotool
windowclose` (which destroys the window out from under the game).
"""
import sys
from Xlib import X, display, protocol
d = display.Display()
w = d.create_resource_object("window", int(sys.argv[1], 0))
wm_protocols = d.intern_atom("WM_PROTOCOLS")
wm_delete = d.intern_atom("WM_DELETE_WINDOW")
ev = protocol.event.ClientMessage(window=w, client_type=wm_protocols, data=(32, [wm_delete, X.CurrentTime, 0, 0, 0]))
w.send_event(ev, event_mask=X.NoEventMask)
d.flush()
