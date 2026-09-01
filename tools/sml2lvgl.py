#!/usr/bin/env python3
"""SML -> LVGL UI XML -> C  (LVGL v9)  converter.

Pipeline
--------
    ui.sml  --(smlconv --to lvgl)-->  generic LVGL XML
             --(normalise)-->         LVGL Pro XML project (project.xml / globals.xml / screens/*.xml)
             --(lved generate | builtin emitter)-->  ui_gen.c / ui_gen.h

Why the normalise step exists
-----------------------------
`smlconv --to lvgl` emits a *generic* LVGL XML (stripped `lv_` prefixes, `<event>`
children, no `<view>` root). LVGL Pro's real schema wants `lv_label`/`lv_button`,
an `<event_cb>` element and a `<view>` wrapper, so the two are not interchangeable.

The final XML -> C step normally runs through the LVGL Pro CLI (`lved generate`),
which requires a license token (LVGL_CLI_TOKEN / --token). When no token is
available we fall back to the built-in emitter in this file, which produces
plain LVGL v9 C for the supported widget/attribute subset.

Usage
-----
    python tools/sml2lvgl.py --sml ui.sml --out build/ui [--name main]
    python tools/sml2lvgl.py --sml ui.sml --out build/ui --smlconv path/to/smlconv
    python tools/sml2lvgl.py --sml ui.sml --out build/ui --lved path/to/lved-cli.js

Env
---
    LVGL_CLI_TOKEN  license token for `lved generate` (optional)
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

# --------------------------------------------------------------------------
# widget vocabulary
# --------------------------------------------------------------------------

# generic tag (from smlconv) -> LVGL Pro tag
WIDGET_TAGS = {
    "obj", "label", "button", "slider", "bar", "switch", "checkbox",
    "arc", "image", "dropdown", "text_input", "text_box", "textarea",
    "roller", "chart", "table", "line", "led", "scale", "spinner",
    "win", "tabview", "tileview", "msgbox", "menu", "calendar",
    "keyboard", "colorwheel", "canvas", "animimg", "qrcode",
}

# LVGL v9 create functions (widget -> fn). Missing means "not supported for C".
# A value of None marks a widget that needs custom handling in CGen.walk (e.g. `tab`,
# which is created via lv_tabview_add_tab(tabview, name) and acts as a container).
C_CREATE = {
    "obj": "lv_obj_create",
    "container": "lv_obj_create",  # LVGL v9: container == obj + flex
    "tab": None,  # handled specially: lv_tabview_add_tab(parent, "name")
    "view": "lv_obj_create",
    "screen": "lv_obj_create",
    "label": "lv_label_create",
    "button": "lv_button_create",
    "slider": "lv_slider_create",
    "bar": "lv_bar_create",
    "switch": "lv_switch_create",
    "checkbox": "lv_checkbox_create",
    "arc": "lv_arc_create",
    "image": "lv_image_create",
    "dropdown": "lv_dropdown_create",
    "textarea": "lv_textarea_create",
    "roller": "lv_roller_create",
    "chart": "lv_chart_create",
    "table": "lv_table_create",
    "line": "lv_line_create",
    "led": "lv_led_create",
    "scale": "lv_scale_create",
    "spinner": "lv_spinner_create",
    "keyboard": "lv_keyboard_create",
    "colorwheel": "lv_colorwheel_create",
    "canvas": "lv_canvas_create",
    "animimg": "lv_animimg_create",
    "win": "lv_win_create",
    "tabview": "lv_tabview_create",
    "tileview": "lv_tileview_create",
    "menu": "lv_menu_create",
    "calendar": "lv_calendar_create",
}

# textarea-ish tags collapse onto lv_textarea for C purposes
C_ALIAS = {"text_input": "textarea", "text_box": "textarea"}

# SML `on_<event>` -> LVGL trigger name
TRIGGER_MAP = {
    "click": "clicked",
    "clicked": "clicked",
    "press": "pressed",
    "pressed": "pressed",
    "release": "released",
    "released": "released",
    "value_changed": "value_changed",
    "change": "value_changed",
    "focused": "focused",
    "defocused": "defocused",
    "short_clicked": "short_clicked",
    "long_pressed": "long_pressed",
}

# LVGL trigger -> C enum
C_EVENT_ENUM = {
    "clicked": "LV_EVENT_CLICKED",
    "pressed": "LV_EVENT_PRESSED",
    "released": "LV_EVENT_RELEASED",
    "value_changed": "LV_EVENT_VALUE_CHANGED",
    "focused": "LV_EVENT_FOCUSED",
    "defocused": "LV_EVENT_DEFOCUSED",
    "short_clicked": "LV_EVENT_SHORT_CLICKED",
    "long_pressed": "LV_EVENT_LONG_PRESSED",
    "screen_loaded": "LV_EVENT_SCREEN_LOADED",
}

C_ALIGN_ENUM = {
    "center": "LV_ALIGN_CENTER",
    "top_left": "LV_ALIGN_TOP_LEFT",
    "top_mid": "LV_ALIGN_TOP_MID",
    "top_right": "LV_ALIGN_TOP_RIGHT",
    "bottom_left": "LV_ALIGN_BOTTOM_LEFT",
    "bottom_mid": "LV_ALIGN_BOTTOM_MID",
    "bottom_right": "LV_ALIGN_BOTTOM_RIGHT",
    "left_mid": "LV_ALIGN_LEFT_MID",
    "right_mid": "LV_ALIGN_RIGHT_MID",
}

C_FLEX_FLOW_ENUM = {
    "row": "LV_FLEX_FLOW_ROW",
    "column": "LV_FLEX_FLOW_COLUMN",
    "row_wrap": "LV_FLEX_FLOW_ROW_WRAP",
    "column_wrap": "LV_FLEX_FLOW_COLUMN_WRAP",
    "row_reverse": "LV_FLEX_FLOW_ROW_REVERSE",
    "column_reverse": "LV_FLEX_FLOW_COLUMN_REVERSE",
}

# --------------------------------------------------------------------------
# step 1: SML -> generic LVGL XML (via smlconv)
# --------------------------------------------------------------------------


def find_smlconv(explicit=None):
    if explicit:
        return explicit
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.dirname(here)
    for cand in (
        os.path.join(root, "rust", "target", "debug", "smlconv.exe"),
        os.path.join(root, "rust", "target", "release", "smlconv.exe"),
        os.path.join(root, "rust", "target", "debug", "smlconv"),
        os.path.join(root, "rust", "target", "release", "smlconv"),
    ):
        if os.path.isfile(cand):
            return cand
    return shutil.which("smlconv")


def sml_to_generic_xml(sml_path, smlconv):
    """Run `smlconv --to lvgl` and return the XML text."""
    if not smlconv:
        raise SystemExit("smlconv not found; build it or pass --smlconv")
    r = subprocess.run([smlconv, "-i", sml_path, "--to", "lvgl"],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise SystemExit(f"smlconv failed ({r.returncode}):\n{r.stderr}")
    return r.stdout


# --------------------------------------------------------------------------
# step 2: generic LVGL XML -> LVGL Pro XML
# --------------------------------------------------------------------------


def _is_widget(tag):
    t = tag[len("lv_"):] if tag.startswith("lv_") else tag
    return t in WIDGET_TAGS


def _pro_tag(tag):
    return tag if tag.startswith("lv_") else "lv_" + tag


def normalise(elem):
    """Rewrite one element tree into LVGL Pro schema."""
    tag = elem.tag

    if tag == "event":
        # <event name="click" handler="X"/> -> <event_cb callback="X" trigger="clicked"/>
        ev = elem.attrib.get("name", "")
        handler = elem.attrib.get("handler", "")
        new = ET.Element("event_cb")
        new.set("callback", handler)
        new.set("trigger", TRIGGER_MAP.get(ev, ev))
        return [new]

    if tag == "screen":
        # children move under a <view> wrapper
        view = ET.Element("view", elem.attrib)
        view.set("width", "100%")
        view.set("height", "100%")
        for child in list(elem):
            view.extend(normalise(child))
        screen = ET.Element("screen")
        if elem.attrib.get("name"):
            screen.set("name", elem.attrib["name"])
        screen.append(view)
        return [screen]

    if _is_widget(tag) or tag in ("view",):
        out = ET.Element(_pro_tag(tag))
        text = None
        for k, v in elem.attrib.items():
            if k == "text" and _pro_tag(tag) != "lv_label":
                text = v  # becomes a child label
                continue
            out.set(k, v)
        for child in list(elem):
            out.extend(normalise(child))
        if text is not None:
            lbl = ET.Element("lv_label")
            lbl.set("text", text)
            lbl.set("align", "center")
            out.append(lbl)
        return [out]

    # unknown: keep as-is, recurse
    out = ET.Element(tag, elem.attrib)
    for child in list(elem):
        out.extend(normalise(child))
    return [out]


def to_pro_xml(generic_xml, screen_name):
    text = generic_xml.strip()
    # strip the xml declaration smlconv emits, ET dislikes it with encoding=
    text = re.sub(r"^<\?xml[^>]*\?>\s*", "", text)
    root = ET.fromstring(text)
    nodes = normalise(root)
    if len(nodes) != 1 or nodes[0].tag != "screen":
        nodes = [n for n in nodes if n.tag == "screen"]
    if not nodes:
        raise SystemExit("no <screen> found in smlconv lvgl output")
    screen = nodes[0]
    if not screen.attrib.get("name"):
        screen.set("name", screen_name)
    ET.indent(screen, space="\t")
    return ET.tostring(screen, encoding="unicode")


# --------------------------------------------------------------------------
# step 3a: LVGL Pro XML project -> C via lved (needs license token)
# --------------------------------------------------------------------------


def write_project(out_dir, screen_xml, screen_name, width, height):
    os.makedirs(os.path.join(out_dir, "screens"), exist_ok=True)
    with open(os.path.join(out_dir, "project.xml"), "w", encoding="utf-8") as f:
        f.write(f'<project lvgl_version="9.5.0">\n'
                f'    <targets>\n'
                f'        <target name="target1">\n'
                f'            <display width="{width}" height="{height}"/>\n'
                f'        </target>\n'
                f'    </targets>\n'
                f'</project>\n')
    with open(os.path.join(out_dir, "globals.xml"), "w", encoding="utf-8") as f:
        f.write("<globals>\n    <api>\n    </api>\n    <consts>\n    </consts>\n"
                "    <subjects>\n    </subjects>\n    <images>\n    </images>\n"
                "    <fonts>\n    </fonts>\n    <styles>\n    </styles>\n</globals>\n")
    with open(os.path.join(out_dir, "screens", f"{screen_name}.xml"), "w", encoding="utf-8") as f:
        f.write(screen_xml + "\n")


def run_lved(lved, out_dir):
    token = os.environ.get("LVGL_CLI_TOKEN")
    cmd = ["node", lved, "generate", out_dir, "--ignore-fonts", "--ignore-images"]
    if token:
        cmd += ["--token", token]
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r


# --------------------------------------------------------------------------
# step 3b: builtin LVGL Pro XML -> C emitter (no license needed)
# --------------------------------------------------------------------------


class CGen:
    def __init__(self, screen_name, width, height):
        self.screen_name = screen_name
        self.width = width
        self.height = height
        self.lines = []
        self.decls = []        # widget handle declarations
        self.callbacks = []    # (fn, trigger)
        self.seen_cb = set()
        self.counter = 0
        self.created = []      # (var, parent_var, widget)
        self.setters = []      # lazy body lines

    def var(self, hint=None):
        self.counter += 1
        # Only ASCII identifiers are safe as C variable names; otherwise fall back
        # to a positional name ui_wN (e.g. when the SML widget name is Chinese).
        if hint and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", hint):
            return f"ui_{hint}"
        return f"ui_w{self.counter}"

    def emit(self, s, indent=1):
        self.lines.append("    " * indent + s)

    # -- attribute helpers -------------------------------------------------
    @staticmethod
    def _size(val):
        """`100%` -> lv_pct(100); `120` -> 120"""
        if isinstance(val, str) and val.endswith("%"):
            n = val[:-1].strip()
            return f"lv_pct({n})" if n.isdigit() else "LV_SIZE_CONTENT"
        if val in ("content", "LV_SIZE_CONTENT"):
            return "LV_SIZE_CONTENT"
        return str(val)

    def walk(self, node, parent_var, depth=0):
        """node: LVGL Pro XML element; returns var name or None."""
        tag = node.tag
        if tag == "event_cb":
            return None

        widget = tag[3:] if tag.startswith("lv_") else tag
        widget = C_ALIAS.get(widget, widget)

        if tag == "screen":
            v = "ui_screen_" + (node.attrib.get("name") or self.screen_name)
            self.decls.append(v)
            self.created.append((v, None, "screen"))
            for child in node:
                self.walk(child, v, depth + 1)
            return v

        if tag == "view":
            v = "ui_view"
            self.decls.append(v)
            self.created.append((v, parent_var, "view"))
            for child in node:
                self.walk(child, v, depth + 1)
            return v

        if widget not in C_CREATE:
            sys.stderr.write(f"warning: widget <{tag}> unsupported by the builtin C emitter, skipped\n")
            return None

        name = node.attrib.get("name")
        v = self.var(name) if name else self.var()
        self.decls.append(v)

        # `tab` is not a stand-alone widget: it is created via
        # lv_tabview_add_tab(parent_tabview, "title") and behaves like a container.
        if widget == "tab":
            title = self.cstr(name or "")
            self.created.append((v, parent_var, f"_tab:{title}"))
            for child in node:
                self.walk(child, v, depth + 1)
            return v

        self.created.append((v, parent_var, widget))

        # attributes -> setter calls
        a = node.attrib
        if "x" in a or "y" in a:
            self.setters.append((v, f"lv_obj_set_pos({v}, {a.get('x', 0)}, {a.get('y', 0)});"))
        if "width" in a or "height" in a:
            w = self._size(a.get("width", "LV_SIZE_CONTENT"))
            h = self._size(a.get("height", "LV_SIZE_CONTENT"))
            self.setters.append((v, f"lv_obj_set_size({v}, {w}, {h});"))
        if "align" in a:
            al = C_ALIGN_ENUM.get(a["align"])
            if al:
                self.setters.append((v, f"lv_obj_set_align({v}, {al});"))
        if "flex_flow" in a:
            ff = C_FLEX_FLOW_ENUM.get(a["flex_flow"])
            if ff:
                self.setters.append((v, f"lv_obj_set_flex_flow({v}, {ff});"))

        # widget-specific
        if widget == "label" and "text" in a:
            self.setters.append((v, f"lv_label_set_text({v}, {self.cstr(a['text'])});"))
        if widget in ("slider", "bar", "arc") and "value" in a:
            self.setters.append((v, f"lv_{widget}_set_value({v}, {a['value']}, LV_ANIM_OFF);"))
        if widget == "checkbox" and "text" in a:
            self.setters.append((v, f"lv_checkbox_set_text({v}, {self.cstr(a['text'])});"))
        if widget == "dropdown" and "options" in a:
            self.setters.append((v, f"lv_dropdown_set_options({v}, {self.cstr(a['options'])});"))
        if widget == "textarea" and "placeholder" in a:
            self.setters.append((v, f"lv_textarea_set_placeholder_text({v}, {self.cstr(a['placeholder'])});"))
        if widget == "image" and "src" in a:
            self.setters.append((v, f"lv_image_set_src({v}, {self.cstr(a['src'])});"))

        for child in node:
            cv = self.walk(child, v, depth + 1)
            if child.tag == "event_cb" and cv is None:
                cb = child.attrib.get("callback", "")
                trig = child.attrib.get("trigger", "clicked")
                if cb:
                    enum = C_EVENT_ENUM.get(trig, "LV_EVENT_CLICKED")
                    self.callbacks.append((cb, enum))
                    self.setters.append((v, f"lv_obj_add_event_cb({v}, {cb}, {enum}, NULL);"))
        return v

    @staticmethod
    def cstr(s):
        esc = s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
        return f'"{esc}"'

    def render(self):
        cb_names = []
        for fn, _ in self.callbacks:
            if fn not in cb_names:
                cb_names.append(fn)

        out = []
        out.append("/* Generated by tools/sml2lvgl.py from SML -- do not edit by hand. */")
        out.append("/* LVGL v9 UI code. Event callbacks are declared here; implement them in your app. */")
        out.append("")
        out.append('#include "lvgl.h"')
        out.append("#include \"ui_gen.h\"")
        out.append("")
        if cb_names:
            out.append("/* ---- event callbacks (to be implemented by the application) ---- */")
            for fn in cb_names:
                out.append(f"void {fn}(lv_event_t * e);")
            out.append("")
        out.append("/* ---- widget handles ---- */")
        for d in self.decls:
            out.append(f"lv_obj_t * {d};")
        out.append("")
        out.append(f"lv_obj_t * ui_{self.screen_name}_create(void)")
        out.append("{")
        # creation
        for v, parent, widget in self.created:
            if widget == "screen":
                out.append(f"    {v} = lv_obj_create(NULL);")
                out.append(f"    lv_obj_set_size({v}, {self.width}, {self.height});")
            elif widget.startswith("_tab:"):
                title = widget[len("_tab:"):]
                out.append(f"    {v} = lv_tabview_add_tab({parent}, {title});")
            else:
                fn = C_CREATE[widget]
                out.append(f"    {v} = {fn}({parent});")
            # interleaved setters for this var
            for sv, line in [x for x in self.setters if x[0] == v]:
                out.append("    " + line)
        out.append("")
        out.append(f"    return ui_screen_{self.screen_name};")
        out.append("}")
        out.append("")
        return "\n".join(out)

    def render_header(self):
        cb_names = []
        for fn, _ in self.callbacks:
            if fn not in cb_names:
                cb_names.append(fn)
        out = []
        guard = "UI_GEN_H"
        out.append(f"#ifndef {guard}")
        out.append(f"#define {guard}")
        out.append("")
        out.append('#include "lvgl.h"')
        out.append("")
        out.append("/* ---- widget handles ---- */")
        for d in self.decls:
            out.append(f"extern lv_obj_t * {d};")
        out.append("")
        out.append(f"lv_obj_t * ui_{self.screen_name}_create(void);")
        out.append("")
        out.append(f"#endif /* {guard} */")
        return "\n".join(out)


def emit_c(pro_xml, screen_name, width, height):
    root = ET.fromstring(pro_xml)
    gen = CGen(screen_name, width, height)
    gen.walk(root, None)
    return gen.render(), gen.render_header()


# --------------------------------------------------------------------------
# main
# --------------------------------------------------------------------------


def main():
    ap = argparse.ArgumentParser(description="SML -> LVGL UI XML -> C")
    ap.add_argument("--sml", required=True, help="input SML file")
    ap.add_argument("--out", required=True, help="output project directory")
    ap.add_argument("--name", default=None, help="screen name (default: SML file stem)")
    ap.add_argument("--width", type=int, default=320)
    ap.add_argument("--height", type=int, default=240)
    ap.add_argument("--smlconv", default=None, help="path to smlconv binary")
    ap.add_argument("--lved", default=None, help="path to lved-cli.js; enables the Pro codegen")
    ap.add_argument("--no-c", action="store_true", help="only emit the LVGL Pro XML project")
    args = ap.parse_args()

    screen_name = args.name or os.path.splitext(os.path.basename(args.sml))[0]
    screen_name = re.sub(r"\W", "_", screen_name)

    print(f"[1/4] SML -> generic LVGL XML  ({args.sml})")
    generic = sml_to_generic_xml(args.sml, find_smlconv(args.smlconv))
    print(generic.strip())

    print("\n[2/4] normalise -> LVGL Pro XML")
    pro = to_pro_xml(generic, screen_name)
    print(pro)

    print(f"\n[3/4] write LVGL Pro project -> {args.out}")
    write_project(args.out, pro, screen_name, args.width, args.height)

    c_path = None
    if args.lved:
        print("\n[4/4] lved generate (LVGL Pro CLI)")
        r = run_lved(args.lved, args.out)
        print(r.stdout[-4000:])
        if r.returncode != 0:
            print(r.stderr[-2000:], file=sys.stderr)
            print("lved failed (license token?); falling back to the builtin emitter", file=sys.stderr)
        else:
            print("lved generate OK")
            return 0
    elif not args.no_c:
        print("\n[4/4] builtin emitter (no LVGL Pro license token)")

    if args.no_c:
        return 0

    c_src, c_hdr = emit_c(pro, screen_name, args.width, args.height)
    c_path = os.path.join(args.out, "ui_gen.c")
    h_path = os.path.join(args.out, "ui_gen.h")
    with open(c_path, "w", encoding="utf-8") as f:
        f.write(c_src)
    with open(h_path, "w", encoding="utf-8") as f:
        f.write(c_hdr)
    print(f"wrote {c_path}")
    print(f"wrote {h_path}")
    print("\n--- ui_gen.c ---")
    print(c_src)
    return 0


if __name__ == "__main__":
    sys.exit(main())
