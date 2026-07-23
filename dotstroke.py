#!/usr/bin/env python3

import json
import math
import tkinter as tk
from dataclasses import dataclass, field
from tkinter import filedialog, messagebox, ttk
from typing import Any, Dict, List, Optional, Tuple


CANVAS_WIDTH = 400
CANVAS_HEIGHT = 240
EDITOR_SCALE = 2


Point = Tuple[float, float]
IntPoint = Tuple[int, int]


def clamp(value: int, low: int, high: int) -> int:
	return max(low, min(high, value))


def snap_value(value: float, mode: str) -> float:
	if mode == "subpixel":
		return value
	if mode == "floor":
		return math.floor(value)
	if mode == "ceil":
		return math.ceil(value)
	return round(value)


def snap_point(point: Point, mode: str) -> Point:
	return (snap_value(point[0], mode), snap_value(point[1], mode))


def transform_point(point: Point, transform: Dict[str, Any]) -> Point:
	x, y = point
	tx = float(transform.get("x", 0.0))
	ty = float(transform.get("y", 0.0))
	sx = float(transform.get("scaleX", 1.0))
	sy = float(transform.get("scaleY", 1.0))
	rot_deg = float(transform.get("rotation", 0.0))
	pivot = transform.get("pivot", [0.0, 0.0])
	px, py = float(pivot[0]), float(pivot[1])

	x = (x - px) * sx
	y = (y - py) * sy
	r = math.radians(rot_deg)
	cos_r = math.cos(r)
	sin_r = math.sin(r)
	rx = x * cos_r - y * sin_r
	ry = x * sin_r + y * cos_r
	return (rx + px + tx, ry + py + ty)


def style_key(style: Dict[str, Any]) -> Tuple[Any, ...]:
	dither = style.get("dither", {})
	phase = dither.get("phase", [0, 0])
	return (
		style.get("blend", "normal"),
		style.get("color", "black"),
		int(style.get("width", 1)),
		style.get("cap", "butt"),
		dither.get("type", "none"),
		float(dither.get("level", 1.0)),
		int(phase[0]),
		int(phase[1]),
		dither.get("anchor", "screen"),
	)


def segments_collinear(a: Tuple[int, int, int, int], b: Tuple[int, int, int, int]) -> bool:
	x1, y1, x2, y2 = a
	x3, y3, x4, y4 = b
	dx = x2 - x1
	dy = y2 - y1
	if dx == 0 and dy == 0:
		return False
	if (x3 - x1) * dy - (y3 - y1) * dx != 0:
		return False
	if (x4 - x1) * dy - (y4 - y1) * dx != 0:
		return False
	return True


def merge_two_collinear(
	a: Tuple[int, int, int, int], b: Tuple[int, int, int, int]
) -> Optional[Tuple[int, int, int, int]]:
	if not segments_collinear(a, b):
		return None
	x1, y1, x2, y2 = a
	dx = x2 - x1
	dy = y2 - y1
	length2 = dx * dx + dy * dy
	if length2 == 0:
		return None

	pts = [(a[0], a[1]), (a[2], a[3]), (b[0], b[1]), (b[2], b[3])]

	def projection_t(p: Tuple[int, int]) -> float:
		return ((p[0] - x1) * dx + (p[1] - y1) * dy) / length2

	sorted_pts = sorted(pts, key=projection_t)
	t_a0 = projection_t((a[0], a[1]))
	t_a1 = projection_t((a[2], a[3]))
	t_b0 = projection_t((b[0], b[1]))
	t_b1 = projection_t((b[2], b[3]))
	a_min, a_max = min(t_a0, t_a1), max(t_a0, t_a1)
	b_min, b_max = min(t_b0, t_b1), max(t_b0, t_b1)

	if a_max < b_min - 1e-9 or b_max < a_min - 1e-9:
		return None
	p0 = sorted_pts[0]
	p1 = sorted_pts[-1]
	return (p0[0], p0[1], p1[0], p1[1])


def merge_collinear_segments(
	segments: List[Tuple[int, int, int, int, Dict[str, Any]]]
) -> List[Tuple[int, int, int, int, Dict[str, Any]]]:
	grouped: Dict[Tuple[Any, ...], List[Tuple[int, int, int, int, Dict[str, Any]]]] = {}
	for seg in segments:
		grouped.setdefault(style_key(seg[4]), []).append(seg)

	merged_all: List[Tuple[int, int, int, int, Dict[str, Any]]] = []
	for _, segs in grouped.items():
		changed = True
		work = segs[:]
		while changed:
			changed = False
			out: List[Tuple[int, int, int, int, Dict[str, Any]]] = []
			consumed = [False] * len(work)
			for i in range(len(work)):
				if consumed[i]:
					continue
				cur = work[i]
				for j in range(i + 1, len(work)):
					if consumed[j]:
						continue
					other = work[j]
					merged = merge_two_collinear(cur[:4], other[:4])
					if merged is not None:
						cur = (merged[0], merged[1], merged[2], merged[3], cur[4])
						consumed[j] = True
						changed = True
				consumed[i] = True
				out.append(cur)
			work = out

		dedup: Dict[Tuple[int, int, int, int, Tuple[Any, ...]], Tuple[int, int, int, int, Dict[str, Any]]] = {}
		for seg in work:
			a = (seg[0], seg[1])
			b = (seg[2], seg[3])
			p0, p1 = (a, b) if a <= b else (b, a)
			key = (p0[0], p0[1], p1[0], p1[1], style_key(seg[4]))
			dedup[key] = seg
		merged_all.extend(dedup.values())
	return merged_all


def remove_duplicate_points(points: List[Point]) -> List[Point]:
	if not points:
		return points
	out = [points[0]]
	for p in points[1:]:
		if p != out[-1]:
			out.append(p)
	return out


def line_bbox(seg: Tuple[int, int, int, int]) -> Tuple[int, int, int, int]:
	x1, y1, x2, y2 = seg
	return (min(x1, x2), min(y1, y2), max(x1, x2), max(y1, y2))


def bbox_outside_screen(bbox: Tuple[int, int, int, int]) -> bool:
	x0, y0, x1, y1 = bbox
	return x1 < 0 or y1 < 0 or x0 >= CANVAS_WIDTH or y0 >= CANVAS_HEIGHT


@dataclass
class VectorObject:
	type: str
	points: List[Point]
	closed: bool = False
	style: Dict[str, Any] = field(default_factory=dict)
	transform: Dict[str, Any] = field(default_factory=dict)

	def to_dict(self) -> Dict[str, Any]:
		return {
			"type": self.type,
			"points": [[p[0], p[1]] for p in self.points],
			"closed": self.closed,
			"style": self.style,
			"transform": self.transform,
		}

	@staticmethod
	def from_dict(data: Dict[str, Any]) -> "VectorObject":
		pts = [(float(p[0]), float(p[1])) for p in data.get("points", [])]
		return VectorObject(
			type=data.get("type", "polyline"),
			points=pts,
			closed=bool(data.get("closed", False)),
			style=data.get("style", {}),
			transform=data.get("transform", {}),
		)


@dataclass
class Layer:
	id: str
	visible: bool = True
	objects: List[VectorObject] = field(default_factory=list)

	def to_dict(self) -> Dict[str, Any]:
		return {
			"id": self.id,
			"visible": self.visible,
			"objects": [obj.to_dict() for obj in self.objects],
		}

	@staticmethod
	def from_dict(data: Dict[str, Any]) -> "Layer":
		return Layer(
			id=data.get("id", "layer"),
			visible=bool(data.get("visible", True)),
			objects=[VectorObject.from_dict(obj) for obj in data.get("objects", [])],
		)


def default_style() -> Dict[str, Any]:
	return {
		"color": "black",
		"blend": "normal",
		"width": 1,
		"cap": "butt",
		"dither": {
			"type": "none",
			"level": 1.0,
			"phase": [0, 0],
			"anchor": "screen",
		},
	}


def new_document() -> Dict[str, Any]:
	return {
		"format": "pdvector",
		"version": 1,
		"target": {
			"sdk": "3.1.1",
			"width": CANVAS_WIDTH,
			"height": CANVAS_HEIGHT,
			"coordinateSystem": "top-left",
			"pixelSnap": "integer",
			"rounding": "nearest",
			"clip": True,
		},
		"canvas": {
			"background": "white",
			"ditherAnchor": "screen",
		},
		"optimize": {
			"mergeCollinearLines": True,
			"removeDuplicatePoints": True,
			"simplifyTolerance": 0,
		},
		"layers": [
			Layer(id="layer1", visible=True, objects=[]).to_dict(),
		],
	}


class Rasterizer:
	def __init__(self, width: int, height: int):
		self.width = width
		self.height = height
		self.clear()

	def clear(self) -> None:
		self.pixels = [[0 for _ in range(self.width)] for _ in range(self.height)]

	def _apply_blend(self, x: int, y: int, style: Dict[str, Any]) -> None:
		if x < 0 or y < 0 or x >= self.width or y >= self.height:
			return
		blend = style.get("blend", "normal")
		color = style.get("color", "black")
		if blend == "xor":
			self.pixels[y][x] = 1 - self.pixels[y][x]
			return
		if color == "black":
			self.pixels[y][x] = 1
		elif color in ("white", "clear"):
			self.pixels[y][x] = 0
		else:
			self.pixels[y][x] = 1

	def _dither_accept(self, x: int, y: int, style: Dict[str, Any]) -> bool:
		d = style.get("dither", {})
		dtype = d.get("type", "none")
		level = float(d.get("level", 1.0))
		phase = d.get("phase", [0, 0])
		px = int(phase[0])
		py = int(phase[1])

		if dtype == "none":
			return True
		if level <= 0.0:
			return False
		if level >= 1.0:
			return True

		if dtype == "checker":
			threshold = 0 if level < 0.5 else 1
			return ((x + px + y + py) & 1) <= threshold
		if dtype in ("line", "screen"):
			period = max(1, int(round(1.0 / max(level, 0.01))))
			return ((y + py) % period) == 0

		return True

	def _stamp(self, cx: int, cy: int, width: int, cap: str, style: Dict[str, Any]) -> None:
		if width <= 1:
			if self._dither_accept(cx, cy, style):
				self._apply_blend(cx, cy, style)
			return

		radius = max(1, int(math.ceil(width / 2.0)))
		for oy in range(-radius, radius + 1):
			for ox in range(-radius, radius + 1):
				if cap == "round" and (ox * ox + oy * oy > radius * radius):
					continue
				x = cx + ox
				y = cy + oy
				if self._dither_accept(x, y, style):
					self._apply_blend(x, y, style)

	def draw_line(self, p0: IntPoint, p1: IntPoint, style: Dict[str, Any]) -> None:
		x0, y0 = p0
		x1, y1 = p1
		dx = x1 - x0
		dy = y1 - y0
		steps = max(abs(dx), abs(dy))
		width = max(1, int(style.get("width", 1)))
		cap = style.get("cap", "butt")
		if steps == 0:
			self._stamp(x0, y0, width, cap, style)
			return

		for i in range(steps + 1):
			t = i / steps
			x = round(x0 + dx * t)
			y = round(y0 + dy * t)
			self._stamp(x, y, width, cap, style)

	def draw_segments(self, segments: List[Tuple[int, int, int, int, Dict[str, Any]]]) -> None:
		for seg in segments:
			x1, y1, x2, y2, style = seg
			self.draw_line((x1, y1), (x2, y2), style)


class DotStrokeApp:
	def __init__(self, root: tk.Tk):
		self.root = root
		self.root.title("DotStroke for Playdate")
		self.doc = new_document()
		self.current_layer_index = 0
		self.current_points: List[Point] = []
		self.status_text = tk.StringVar(value="Ready")

		self.tool_var = tk.StringVar(value="line")
		self.rounding_var = tk.StringVar(value="nearest")
		self.mode_var = tk.StringVar(value="readable")
		self.output_type_var = tk.StringVar(value="line-only")
		self.color_var = tk.StringVar(value="black")
		self.blend_var = tk.StringVar(value="normal")
		self.cap_var = tk.StringVar(value="butt")
		self.width_var = tk.IntVar(value=1)
		self.layer_visible_var = tk.BooleanVar(value=True)

		self.rasterizer = Rasterizer(CANVAS_WIDTH, CANVAS_HEIGHT)
		self.preview_image = tk.PhotoImage(width=CANVAS_WIDTH, height=CANVAS_HEIGHT)
		self.preview_zoom = 2

		self._build_ui()
		self.refresh_layer_list()
		self.redraw_all()

	def _build_ui(self) -> None:
		container = ttk.Frame(self.root, padding=8)
		container.pack(fill=tk.BOTH, expand=True)

		left = ttk.Frame(container)
		left.pack(side=tk.LEFT, fill=tk.Y)
		center = ttk.Frame(container)
		center.pack(side=tk.LEFT, fill=tk.BOTH, expand=True, padx=8)
		right = ttk.Frame(container)
		right.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

		ttk.Label(left, text="Tool").pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.tool_var,
			values=["line", "polyline", "polygon"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(0, 8))

		ttk.Label(left, text="Style").pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.color_var,
			values=["black", "white", "clear"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.blend_var,
			values=["normal", "xor"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(2, 0))
		ttk.Combobox(
			left,
			textvariable=self.cap_var,
			values=["butt", "round", "square"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(2, 0))
		ttk.Spinbox(left, from_=1, to=8, textvariable=self.width_var, width=6).pack(anchor=tk.W, pady=(2, 8))

		ttk.Label(left, text="Snap").pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.rounding_var,
			values=["floor", "ceil", "nearest", "subpixel"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(0, 8))

		ttk.Label(left, text="Layer").pack(anchor=tk.W)
		self.layer_list = tk.Listbox(left, height=6, width=18)
		self.layer_list.pack(anchor=tk.W)
		self.layer_list.bind("<<ListboxSelect>>", self.on_layer_select)
		ttk.Checkbutton(
			left,
			text="Visible",
			variable=self.layer_visible_var,
			command=self.on_layer_visible_toggle,
		).pack(anchor=tk.W, pady=(4, 4))
		layer_btns = ttk.Frame(left)
		layer_btns.pack(anchor=tk.W, pady=(0, 8))
		ttk.Button(layer_btns, text="Add", command=self.add_layer).pack(side=tk.LEFT)
		ttk.Button(layer_btns, text="Delete", command=self.delete_layer).pack(side=tk.LEFT, padx=(4, 0))

		ttk.Label(left, text="Export").pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.mode_var,
			values=["readable", "compact", "module"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W)
		ttk.Combobox(
			left,
			textvariable=self.output_type_var,
			values=["line-only", "sdk-native"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(2, 8))

		file_btns = ttk.Frame(left)
		file_btns.pack(anchor=tk.W, pady=(0, 8))
		ttk.Button(file_btns, text="New", command=self.on_new).pack(side=tk.LEFT)
		ttk.Button(file_btns, text="Load JSON", command=self.on_load_json).pack(side=tk.LEFT, padx=(4, 0))
		ttk.Button(file_btns, text="Save JSON", command=self.on_save_json).pack(side=tk.LEFT, padx=(4, 0))

		action_btns = ttk.Frame(left)
		action_btns.pack(anchor=tk.W)
		ttk.Button(action_btns, text="Export Lua", command=self.on_export_lua).pack(side=tk.LEFT)
		ttk.Button(action_btns, text="Optimize", command=self.on_optimize_preview).pack(side=tk.LEFT, padx=(4, 0))

		self.editor_canvas = tk.Canvas(
			center,
			width=CANVAS_WIDTH * EDITOR_SCALE,
			height=CANVAS_HEIGHT * EDITOR_SCALE,
			bg="white",
			highlightthickness=1,
			highlightbackground="#333",
		)
		self.editor_canvas.pack(fill=tk.BOTH, expand=True)
		self.editor_canvas.bind("<Button-1>", self.on_canvas_left_click)
		self.editor_canvas.bind("<Button-3>", self.on_canvas_right_click)
		self.editor_canvas.bind("<Motion>", self.on_canvas_motion)
		self.root.bind("<Return>", self.on_finalize_key)
		self.root.bind("<Escape>", self.on_cancel_key)

		ttk.Label(right, text="1-bit Preview").pack(anchor=tk.W)
		self.preview_label = ttk.Label(right, image=self.preview_image.zoom(self.preview_zoom, self.preview_zoom))
		self.preview_label.pack(anchor=tk.NW)
		ttk.Separator(right, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=6)
		ttk.Label(right, text="Status").pack(anchor=tk.W)
		ttk.Label(right, textvariable=self.status_text, wraplength=360, justify=tk.LEFT).pack(anchor=tk.W)

	def get_layers(self) -> List[Layer]:
		return [Layer.from_dict(layer) for layer in self.doc.get("layers", [])]

	def set_layers(self, layers: List[Layer]) -> None:
		self.doc["layers"] = [layer.to_dict() for layer in layers]

	def current_layer(self) -> Layer:
		layers = self.get_layers()
		self.current_layer_index = clamp(self.current_layer_index, 0, len(layers) - 1)
		return layers[self.current_layer_index]

	def set_status(self, text: str) -> None:
		self.status_text.set(text)

	def refresh_layer_list(self) -> None:
		layers = self.get_layers()
		self.layer_list.delete(0, tk.END)
		for i, layer in enumerate(layers):
			marker = "[x]" if layer.visible else "[ ]"
			self.layer_list.insert(tk.END, f"{marker} {layer.id}")
		if layers:
			self.current_layer_index = clamp(self.current_layer_index, 0, len(layers) - 1)
			self.layer_list.selection_clear(0, tk.END)
			self.layer_list.selection_set(self.current_layer_index)
			self.layer_visible_var.set(layers[self.current_layer_index].visible)

	def add_layer(self) -> None:
		layers = self.get_layers()
		new_id = f"layer{len(layers) + 1}"
		layers.append(Layer(id=new_id, visible=True, objects=[]))
		self.set_layers(layers)
		self.current_layer_index = len(layers) - 1
		self.refresh_layer_list()
		self.redraw_all()

	def delete_layer(self) -> None:
		layers = self.get_layers()
		if len(layers) <= 1:
			messagebox.showinfo("DotStroke", "At least one layer is required.")
			return
		del layers[self.current_layer_index]
		self.set_layers(layers)
		self.current_layer_index = clamp(self.current_layer_index, 0, len(layers) - 1)
		self.refresh_layer_list()
		self.redraw_all()

	def on_layer_select(self, _event: Any) -> None:
		sel = self.layer_list.curselection()
		if not sel:
			return
		self.current_layer_index = sel[0]
		layers = self.get_layers()
		self.layer_visible_var.set(layers[self.current_layer_index].visible)
		self.redraw_all()

	def on_layer_visible_toggle(self) -> None:
		layers = self.get_layers()
		if not layers:
			return
		layers[self.current_layer_index].visible = bool(self.layer_visible_var.get())
		self.set_layers(layers)
		self.refresh_layer_list()
		self.redraw_all()

	def _editor_to_doc_point(self, event: tk.Event) -> Point:
		return (event.x / EDITOR_SCALE, event.y / EDITOR_SCALE)

	def _active_style(self) -> Dict[str, Any]:
		style = default_style()
		style["color"] = self.color_var.get()
		style["blend"] = self.blend_var.get()
		style["cap"] = self.cap_var.get()
		style["width"] = max(1, int(self.width_var.get()))
		return style

	def on_canvas_left_click(self, event: tk.Event) -> None:
		p = self._editor_to_doc_point(event)
		rounded = snap_point(p, self.rounding_var.get())
		tool = self.tool_var.get()

		if tool == "line":
			self.current_points.append(rounded)
			if len(self.current_points) == 2:
				self._commit_object("line", self.current_points[:], False)
				self.current_points.clear()
		elif tool in ("polyline", "polygon"):
			self.current_points.append(rounded)
		self.redraw_all()

	def on_canvas_right_click(self, _event: tk.Event) -> None:
		self.finalize_current_shape()

	def on_finalize_key(self, _event: tk.Event) -> None:
		self.finalize_current_shape()

	def on_cancel_key(self, _event: tk.Event) -> None:
		self.current_points.clear()
		self.set_status("Current drawing canceled.")
		self.redraw_all()

	def on_canvas_motion(self, event: tk.Event) -> None:
		p = self._editor_to_doc_point(event)
		px = int(clamp(round(p[0]), 0, CANVAS_WIDTH - 1))
		py = int(clamp(round(p[1]), 0, CANVAS_HEIGHT - 1))
		self.set_status(
			f"Cursor: ({px}, {py}) | Tool: {self.tool_var.get()} | Pending points: {len(self.current_points)}"
		)

	def finalize_current_shape(self) -> None:
		tool = self.tool_var.get()
		if tool == "polyline" and len(self.current_points) >= 2:
			self._commit_object("polyline", self.current_points[:], False)
			self.current_points.clear()
		elif tool == "polygon" and len(self.current_points) >= 3:
			self._commit_object("polygon", self.current_points[:], True)
			self.current_points.clear()
		else:
			self.set_status("Not enough points to finalize shape.")
		self.redraw_all()

	def _commit_object(self, obj_type: str, points: List[Point], closed: bool) -> None:
		layers = self.get_layers()
		layer = layers[self.current_layer_index]
		if self.doc.get("optimize", {}).get("removeDuplicatePoints", True):
			points = remove_duplicate_points(points)
		if len(points) < 2:
			self.set_status("Object ignored: too few points.")
			return
		if obj_type == "polygon" and len(points) < 3:
			self.set_status("Polygon ignored: at least 3 points required.")
			return

		obj = VectorObject(
			type=obj_type,
			points=points,
			closed=closed,
			style=self._active_style(),
			transform={
				"x": 0,
				"y": 0,
				"scaleX": 1,
				"scaleY": 1,
				"rotation": 0,
				"pivot": [0, 0],
			},
		)
		layer.objects.append(obj)
		layers[self.current_layer_index] = layer
		self.set_layers(layers)
		self.set_status(f"Added {obj_type} with {len(points)} points.")
		self.refresh_layer_list()

	def _object_to_poly_points(self, obj: VectorObject) -> List[IntPoint]:
		snap_mode = self.rounding_var.get()
		pts: List[IntPoint] = []
		for p in obj.points:
			tp = transform_point(p, obj.transform)
			sp = snap_point(tp, snap_mode)
			pts.append((int(round(sp[0])), int(round(sp[1]))))
		return pts

	def _build_segments(
		self,
		optimize: bool,
		output_type: str,
	) -> List[Tuple[int, int, int, int, Dict[str, Any]]]:
		layers = self.get_layers()
		segments: List[Tuple[int, int, int, int, Dict[str, Any]]] = []

		for layer in layers:
			if not layer.visible:
				continue
			for obj in layer.objects:
				pts = self._object_to_poly_points(obj)
				if len(pts) < 2:
					continue
				st = obj.style if obj.style else default_style()

				if obj.type in ("line", "polyline", "path", "bezier"):
					for i in range(len(pts) - 1):
						seg = (pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1], st)
						if not bbox_outside_screen(line_bbox(seg[:4])):
							segments.append(seg)
				elif obj.type in ("polygon", "rect", "ellipse", "group"):
					for i in range(len(pts) - 1):
						seg = (pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1], st)
						if not bbox_outside_screen(line_bbox(seg[:4])):
							segments.append(seg)
					if obj.closed and len(pts) >= 3:
						seg = (pts[-1][0], pts[-1][1], pts[0][0], pts[0][1], st)
						if not bbox_outside_screen(line_bbox(seg[:4])):
							segments.append(seg)
				elif output_type == "sdk-native" and obj.type == "pixel" and len(pts) >= 1:
					seg = (pts[0][0], pts[0][1], pts[0][0], pts[0][1], st)
					if not bbox_outside_screen(line_bbox(seg[:4])):
						segments.append(seg)

		if optimize and self.doc.get("optimize", {}).get("mergeCollinearLines", True):
			segments = merge_collinear_segments(segments)

		return segments

	def redraw_all(self) -> None:
		self.editor_canvas.delete("all")
		self._draw_editor_grid()
		self._draw_editor_objects()
		self._draw_pending_shape()
		self._render_preview()

	def _draw_editor_grid(self) -> None:
		for x in range(0, CANVAS_WIDTH + 1, 20):
			sx = x * EDITOR_SCALE
			self.editor_canvas.create_line(sx, 0, sx, CANVAS_HEIGHT * EDITOR_SCALE, fill="#efefef")
		for y in range(0, CANVAS_HEIGHT + 1, 20):
			sy = y * EDITOR_SCALE
			self.editor_canvas.create_line(0, sy, CANVAS_WIDTH * EDITOR_SCALE, sy, fill="#efefef")

	def _draw_editor_objects(self) -> None:
		segments = self._build_segments(optimize=False, output_type=self.output_type_var.get())
		for x1, y1, x2, y2, style in segments:
			color = style.get("color", "black")
			blend = style.get("blend", "normal")
			if blend == "xor":
				draw_color = "#ff2c95"
			elif color == "black":
				draw_color = "black"
			elif color == "white":
				draw_color = "#9b9b9b"
			elif color == "clear":
				draw_color = "#3f9cff"
			else:
				draw_color = "black"
			self.editor_canvas.create_line(
				x1 * EDITOR_SCALE,
				y1 * EDITOR_SCALE,
				x2 * EDITOR_SCALE,
				y2 * EDITOR_SCALE,
				fill=draw_color,
				width=max(1, int(style.get("width", 1)) * EDITOR_SCALE),
			)

	def _draw_pending_shape(self) -> None:
		if not self.current_points:
			return
		pts = [(p[0] * EDITOR_SCALE, p[1] * EDITOR_SCALE) for p in self.current_points]
		if len(pts) == 1:
			x, y = pts[0]
			self.editor_canvas.create_oval(x - 2, y - 2, x + 2, y + 2, fill="#333", outline="")
			return
		flat: List[float] = []
		for p in pts:
			flat.extend([p[0], p[1]])
		self.editor_canvas.create_line(*flat, fill="#f06", dash=(3, 2))

	def _render_preview(self) -> None:
		self.rasterizer.clear()
		segments = self._build_segments(optimize=True, output_type=self.output_type_var.get())
		self.rasterizer.draw_segments(segments)

		# Tk PhotoImage.put expects rows of explicit color tokens, not run-length tuples.
		rows: List[str] = []
		for y in range(CANVAS_HEIGHT):
			row_colors = ["#000000" if self.rasterizer.pixels[y][x] else "#ffffff" for x in range(CANVAS_WIDTH)]
			rows.append("{" + " ".join(row_colors) + "}")
		self.preview_image.put(" ".join(rows), to=(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT))
		zoomed = self.preview_image.zoom(self.preview_zoom, self.preview_zoom)
		self.preview_label.configure(image=zoomed)
		self.preview_label.image = zoomed

	def on_new(self) -> None:
		if messagebox.askyesno("DotStroke", "Create a new document?"):
			self.doc = new_document()
			self.current_layer_index = 0
			self.current_points.clear()
			self.refresh_layer_list()
			self.redraw_all()
			self.set_status("New document created.")

	def on_save_json(self) -> None:
		self.doc["target"]["rounding"] = self.rounding_var.get()
		path = filedialog.asksaveasfilename(
			title="Save pdvector JSON",
			defaultextension=".json",
			filetypes=[("JSON", "*.json"), ("All files", "*.*")],
		)
		if not path:
			return
		with open(path, "w", encoding="utf-8") as f:
			json.dump(self.doc, f, indent=2)
		self.set_status(f"Saved JSON: {path}")

	def on_load_json(self) -> None:
		path = filedialog.askopenfilename(
			title="Load pdvector JSON",
			filetypes=[("JSON", "*.json"), ("All files", "*.*")],
		)
		if not path:
			return
		try:
			with open(path, "r", encoding="utf-8") as f:
				doc = json.load(f)
			if doc.get("format") != "pdvector":
				raise ValueError("Unsupported format")
			if not doc.get("layers"):
				raise ValueError("No layers found")
			self.doc = doc
			self.current_layer_index = 0
			self.rounding_var.set(self.doc.get("target", {}).get("rounding", "nearest"))
			self.current_points.clear()
			self.refresh_layer_list()
			self.redraw_all()
			self.set_status(f"Loaded JSON: {path}")
		except Exception as exc:
			messagebox.showerror("DotStroke", f"Failed to load JSON: {exc}")

	def on_optimize_preview(self) -> None:
		before = len(self._build_segments(optimize=False, output_type=self.output_type_var.get()))
		after = len(self._build_segments(optimize=True, output_type=self.output_type_var.get()))
		self.redraw_all()
		self.set_status(f"Optimization preview: {before} segments -> {after} segments")

	def _lua_color_expr(self, style: Dict[str, Any]) -> str:
		blend = style.get("blend", "normal")
		if blend == "xor":
			return "gfx.kColorXOR"
		color = style.get("color", "black")
		if color == "white":
			return "gfx.kColorWhite"
		if color == "clear":
			return "gfx.kColorClear"
		return "gfx.kColorBlack"

	def _lua_cap_expr(self, cap: str) -> str:
		mapping = {
			"butt": "gfx.kLineCapStyleButt",
			"round": "gfx.kLineCapStyleRound",
			"square": "gfx.kLineCapStyleSquare",
		}
		return mapping.get(cap, "gfx.kLineCapStyleButt")

	def _lua_dither_lines(self, style: Dict[str, Any]) -> List[str]:
		d = style.get("dither", {})
		dtype = d.get("type", "none")
		level = float(d.get("level", 1.0))
		if dtype == "none" or level >= 1.0:
			return ["    -- clear dither state by resetting solid color", "    gfx.setColor(currentColor)"]
		if style.get("blend") == "xor":
			return ["    -- xor + dither is unsupported in Playdate primitives"]
		mapping = {
			"checker": "gfx.image.kDitherTypeBayer2x2",
			"line": "gfx.image.kDitherTypeHorizontalLine",
			"screen": "gfx.image.kDitherTypeScreen",
			"bayer2x2": "gfx.image.kDitherTypeBayer2x2",
			"bayer4x4": "gfx.image.kDitherTypeBayer4x4",
			"bayer8x8": "gfx.image.kDitherTypeBayer8x8",
		}
		dither_expr = mapping.get(dtype, "gfx.image.kDitherTypeBayer8x8")
		return [f"    gfx.setDitherPattern({level:.3f}, {dither_expr})"]

	def _generate_lua(self) -> str:
		mode = self.mode_var.get()
		output_type = self.output_type_var.get()
		segments = self._build_segments(optimize=True, output_type=output_type)

		def lua_quote(value: str) -> str:
			return "'" + value.replace("\\", "\\\\").replace("'", "\\'") + "'"

		grouped: Dict[Tuple[Any, ...], List[Tuple[int, int, int, int, Dict[str, Any]]]] = {}
		for seg in segments:
			grouped.setdefault(style_key(seg[4]), []).append(seg)

		lines: List[str] = []
		lines.append("local gfx <const> = playdate.graphics")
		lines.append("")
		lines.append("local function resetGraphicsState()")
		lines.append("    gfx.setColor(gfx.kColorBlack)")
		lines.append("    gfx.setLineWidth(1)")
		lines.append("    gfx.setLineCapStyle(gfx.kLineCapStyleButt)")
		lines.append("end")
		lines.append("")

		if mode == "compact":
			lines.append("local styles = {")
			style_table: List[Tuple[Tuple[Any, ...], Dict[str, Any]]] = []
			for key, segs in grouped.items():
				style_table.append((key, segs[0][4]))
			for _, st in style_table:
				lines.append(
					f"    {{ color = {self._lua_color_expr(st)}, width = {int(st.get('width', 1))}, cap = {lua_quote(st.get('cap', 'butt'))} }},"
				)
			lines.append("}")
			lines.append("")
			lines.append("local segments = {")
			key_to_idx = {key: idx + 1 for idx, (key, _) in enumerate(style_table)}
			for seg in segments:
				idx = key_to_idx[style_key(seg[4])]
				lines.append(f"    {{{seg[0]}, {seg[1]}, {seg[2]}, {seg[3]}, {idx}}},")
			lines.append("}")
			lines.append("")

		if mode == "module":
			lines.append("local iconData = {")
			lines.append("    segments = {")
			for seg in segments:
				st = seg[4]
				lines.append(
					"        { x1=%d, y1=%d, x2=%d, y2=%d, color=%s, blend=%s, width=%d, cap=%s },"
					% (
						seg[0],
						seg[1],
						seg[2],
						seg[3],
						lua_quote(st.get("color", "black")),
						lua_quote(st.get("blend", "normal")),
						int(st.get("width", 1)),
						lua_quote(st.get("cap", "butt")),
					)
				)
			lines.append("    },")
			lines.append("}")
			lines.append("")

		lines.append("function drawIcon(x, y, opts)")
		lines.append("    opts = opts or {}")
		lines.append("    if not opts.preserveGraphicsState then")
		lines.append("        resetGraphicsState()")
		lines.append("    end")
		lines.append("")

		if mode == "readable":
			for key, segs in grouped.items():
				del key
				st = segs[0][4]
				lines.append(f"    local currentColor = {self._lua_color_expr(st)}")
				lines.append("    gfx.setColor(currentColor)")
				lines.append(f"    gfx.setLineWidth({int(st.get('width', 1))})")
				lines.append(f"    gfx.setLineCapStyle({self._lua_cap_expr(st.get('cap', 'butt'))})")
				lines.extend(self._lua_dither_lines(st))
				for seg in segs:
					lines.append(f"    gfx.drawLine(x + {seg[0]}, y + {seg[1]}, x + {seg[2]}, y + {seg[3]})")
				lines.append("")

		elif mode == "compact":
			lines.append("    for _, seg in ipairs(segments) do")
			lines.append("        local st = styles[seg[5]]")
			lines.append("        gfx.setColor(st.color)")
			lines.append("        gfx.setLineWidth(st.width)")
			lines.append("        if st.cap == 'round' then")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleRound)")
			lines.append("        elseif st.cap == 'square' then")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleSquare)")
			lines.append("        else")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleButt)")
			lines.append("        end")
			lines.append("        gfx.drawLine(x + seg[1], y + seg[2], x + seg[3], y + seg[4])")
			lines.append("    end")
			lines.append("")

		elif mode == "module":
			lines.append("    for _, s in ipairs(iconData.segments) do")
			lines.append("        local color = gfx.kColorBlack")
			lines.append("        if s.blend == 'xor' then")
			lines.append("            color = gfx.kColorXOR")
			lines.append("        elseif s.color == 'white' then")
			lines.append("            color = gfx.kColorWhite")
			lines.append("        elseif s.color == 'clear' then")
			lines.append("            color = gfx.kColorClear")
			lines.append("        end")
			lines.append("        gfx.setColor(color)")
			lines.append("        gfx.setLineWidth(s.width)")
			lines.append("        if s.cap == 'round' then")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleRound)")
			lines.append("        elseif s.cap == 'square' then")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleSquare)")
			lines.append("        else")
			lines.append("            gfx.setLineCapStyle(gfx.kLineCapStyleButt)")
			lines.append("        end")
			if output_type == "sdk-native":
				lines.append("        -- sdk-native mode currently emits line primitives for MVP compatibility")
			lines.append("        gfx.drawLine(x + s.x1, y + s.y1, x + s.x2, y + s.y2)")
			lines.append("    end")
			lines.append("")

		lines.append("    if not opts.preserveGraphicsState then")
		lines.append("        resetGraphicsState()")
		lines.append("    end")
		lines.append("end")
		lines.append("")
		if mode == "module":
			lines.append("return {")
			lines.append("    data = iconData,")
			lines.append("    draw = drawIcon,")
			lines.append("}")

		return "\n".join(lines)

	def on_export_lua(self) -> None:
		lua_code = self._generate_lua()
		path = filedialog.asksaveasfilename(
			title="Export Lua",
			defaultextension=".lua",
			filetypes=[("Lua", "*.lua"), ("All files", "*.*")],
		)
		if path:
			with open(path, "w", encoding="utf-8") as f:
				f.write(lua_code)
			self.set_status(f"Exported Lua: {path}")

		preview = tk.Toplevel(self.root)
		preview.title("Lua Output")
		text = tk.Text(preview, width=96, height=36, wrap=tk.NONE)
		text.pack(fill=tk.BOTH, expand=True)
		text.insert("1.0", lua_code)
		text.configure(state=tk.DISABLED)


def main() -> None:
	root = tk.Tk()
	app = DotStrokeApp(root)
	del app
	root.mainloop()


if __name__ == "__main__":
	main()
