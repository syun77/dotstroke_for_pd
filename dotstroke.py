#!/usr/bin/env python3

import json
import math
import copy
import tkinter as tk
from dataclasses import dataclass, field
from tkinter import filedialog, messagebox, ttk
from typing import Any, Dict, Iterable, List, Optional, Tuple


CANVAS_WIDTH = 400
CANVAS_HEIGHT = 240
EDITOR_SCALE = 2


Point = Tuple[float, float]
IntPoint = Tuple[int, int]
Segment = Tuple[int, int, int, int, Dict[str, Any]]


def copy_style(style: Optional[Dict[str, Any]]) -> Dict[str, Any]:
	"""Return a normalized style without mutating the document."""
	base = default_style()
	if style:
		base.update(style)
	base["width"] = max(1, int(base.get("width", 1)))
	base["dither"] = {**base["dither"], **(style or {}).get("dither", {})}
	return base


def clamp(value: int, low: int, high: int) -> int:
	return max(low, min(high, value))


def snap_value(value: float, mode: str) -> float:
	if mode == "subpixel":
		return value
	if mode == "floor":
		return math.floor(value)
	if mode == "ceil":
		return math.ceil(value)
	# Python's round uses bankers rounding, which is surprising for an editor
	# coordinate.  Playdate coordinates use the conventional half-up rule.
	return math.floor(value + 0.5)


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


def compose_transforms(parent: Dict[str, Any], local: Dict[str, Any]) -> Dict[str, Any]:
	"""Compose transforms by transforming the local basis and origin.

	The editor stores transforms as translate/scale/rotate/pivot records.  A
	full matrix is used internally so nested groups behave correctly.
	"""
	def apply(p: Point, tr: Dict[str, Any]) -> Point:
		return apply_transform(p, tr)

	origin = apply((0.0, 0.0), local)
	px = apply((1.0, 0.0), local)
	py = apply((0.0, 1.0), local)
	origin = apply(origin, parent)
	px = apply(px, parent)
	py = apply(py, parent)
	return {
		"_matrix": [
			px[0] - origin[0], py[0] - origin[0], origin[0],
			px[1] - origin[1], py[1] - origin[1], origin[1],
		],
	}


def apply_transform(point: Point, transform: Dict[str, Any]) -> Point:
	matrix = transform.get("_matrix")
	if matrix:
		a, b, c, d, e, f = matrix
		return (a * point[0] + b * point[1] + c, d * point[0] + e * point[1] + f)
	return transform_point(point, transform)


def inverse_transform_point(point: Point, transform: Dict[str, Any]) -> Point:
	if transform.get("_matrix"):
		a, b, c, d, e, f = transform["_matrix"]
		determinant = a * e - b * d
		if abs(determinant) < 1e-12:
			return point
		x, y = point[0] - c, point[1] - f
		return ((e * x - b * y) / determinant, (-d * x + a * y) / determinant)
	tx = float(transform.get("x", 0.0))
	ty = float(transform.get("y", 0.0))
	sx = float(transform.get("scaleX", 1.0))
	sy = float(transform.get("scaleY", 1.0))
	px, py = [float(v) for v in transform.get("pivot", [0.0, 0.0])]
	x, y = point[0] - tx - px, point[1] - ty - py
	r = math.radians(float(transform.get("rotation", 0.0)))
	cos_r, sin_r = math.cos(r), math.sin(r)
	rx, ry = x * cos_r + y * sin_r, -x * sin_r + y * cos_r
	return (rx / sx + px if sx else px, ry / sy + py if sy else py)


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


def merge_collinear_segments(segments: List[Segment]) -> List[Segment]:
	"""Merge only adjacent compatible segments and preserve draw order.

	The old implementation grouped the entire document by style.  That made
	XOR and white/clear drawing incorrect because Playdate drawing is ordered.
	"""
	if not segments:
		return []
	out: List[Segment] = []
	for seg in segments:
		merge_safe = (
			seg[4].get("blend", "normal") != "xor"
			and seg[4].get("cap", "butt") == "butt"
		)
		if out and merge_safe and style_key(out[-1][4]) == style_key(seg[4]):
			previous = out[-1]
			merged = merge_two_collinear(previous[:4], seg[:4])
			if merged is not None:
				out[-1] = (*merged, previous[4])
				continue
		# Drawing an XOR segment twice is meaningful, so do not deduplicate it.
		if seg[4].get("blend", "normal") != "xor":
			duplicate = any(
				style_key(existing[4]) == style_key(seg[4])
				and {existing[:4]} == {seg[:4]}
				for existing in out
			)
			if duplicate:
				continue
		out.append(seg)
	return out


def remove_duplicate_points(points: List[Point]) -> List[Point]:
	if not points:
		return points
	out = [points[0]]
	for p in points[1:]:
		if p != out[-1]:
			out.append(p)
	return out


def simplify_points(points: List[Point], tolerance: float) -> List[Point]:
	"""Ramer-Douglas-Peucker simplification for optional export optimization."""
	if tolerance <= 0 or len(points) <= 2:
		return points
	x1, y1 = points[0]
	x2, y2 = points[-1]
	max_distance = -1.0
	max_index = 0
	for i, (x, y) in enumerate(points[1:-1], start=1):
		if (x1, y1) == (x2, y2):
			distance = math.hypot(x - x1, y - y1)
		else:
			dx, dy = x2 - x1, y2 - y1
			distance = abs(dy * x - dx * y + x2 * y1 - y2 * x1) / math.hypot(dx, dy)
		if distance > max_distance:
			max_distance, max_index = distance, i
	if max_distance > tolerance:
		left = simplify_points(points[: max_index + 1], tolerance)
		right = simplify_points(points[max_index:], tolerance)
		return left[:-1] + right
	return [points[0], points[-1]]


def polygon_scanlines(
	points: List[IntPoint], width: int = CANVAS_WIDTH, height: int = CANVAS_HEIGHT
) -> Iterable[Tuple[int, int, int]]:
	"""Yield inclusive horizontal spans for an integer polygon."""
	if len(points) < 3:
		return
	y_min = max(0, min(p[1] for p in points))
	y_max = min(height - 1, max(p[1] for p in points))
	for y in range(y_min, y_max + 1):
		xs: List[float] = []
		for a, b in zip(points, points[1:] + points[:1]):
			x1, y1 = a
			x2, y2 = b
			if y1 == y2:
				continue
			if min(y1, y2) <= y < max(y1, y2):
				xs.append(x1 + (y - y1) * (x2 - x1) / (y2 - y1))
		for left, right in zip(sorted(xs)[::2], sorted(xs)[1::2]):
			x0 = math.ceil(min(left, right))
			x1 = math.floor(max(left, right))
			if x0 <= x1:
				yield x0, x1, y


def line_bbox(seg: Tuple[int, int, int, int]) -> Tuple[int, int, int, int]:
	x1, y1, x2, y2 = seg
	return (min(x1, x2), min(y1, y2), max(x1, x2), max(y1, y2))


def bbox_outside_screen(
	bbox: Tuple[float, float, float, float], width: int = CANVAS_WIDTH, height: int = CANVAS_HEIGHT
) -> bool:
	x0, y0, x1, y1 = bbox
	return x1 < 0 or y1 < 0 or x0 >= width or y0 >= height


@dataclass
class VectorObject:
	type: str
	points: List[Point]
	closed: bool = False
	style: Dict[str, Any] = field(default_factory=dict)
	transform: Dict[str, Any] = field(default_factory=dict)
	children: List["VectorObject"] = field(default_factory=list)
	visible: bool = True

	def to_dict(self) -> Dict[str, Any]:
		return {
			"type": self.type,
			"points": [[p[0], p[1]] for p in self.points],
			"closed": self.closed,
			"style": self.style,
			"transform": self.transform,
			"visible": self.visible,
			**({"children": [child.to_dict() for child in self.children]} if self.children else {}),
		}

	@staticmethod
	def from_dict(data: Dict[str, Any]) -> "VectorObject":
		pts = [(float(p[0]), float(p[1])) for p in data.get("points", [])]
		return VectorObject(
			type=data.get("type", "polyline"),
			points=pts,
			closed=bool(data.get("closed", False)),
			style=copy_style(data.get("style", {})),
			transform=data.get("transform", {}),
			children=[VectorObject.from_dict(child) for child in data.get("children", [])],
			visible=bool(data.get("visible", True)),
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
		"fill": False,
		"dither": {
			"type": "none",
			"level": 1.0,
			"phase": [0, 0],
			"anchor": "screen",
		},
	}


SUPPORTED_OBJECT_TYPES = {"pixel", "line", "polyline", "polygon", "rect", "ellipse", "bezier", "path", "group"}


def normalize_document(raw: Dict[str, Any]) -> Dict[str, Any]:
	if raw.get("format") != "pdvector":
		raise ValueError("Unsupported format")
	target = {**new_document()["target"], **raw.get("target", {})}
	if int(target.get("width", 0)) <= 0 or int(target.get("height", 0)) <= 0:
		raise ValueError("Canvas width and height must be positive")
	if target.get("rounding", "nearest") not in {"floor", "ceil", "nearest", "subpixel"}:
		raise ValueError("Unsupported rounding mode")
	canvas = {**new_document()["canvas"], **raw.get("canvas", {})}
	if canvas.get("ditherAnchor", "screen") not in {"screen", "object"}:
		raise ValueError("Unsupported dither anchor")
	layers: List[Dict[str, Any]] = []
	for layer_data in raw.get("layers", []):
		layer = Layer.from_dict(layer_data)
		def validate(obj: VectorObject) -> None:
			if obj.type not in SUPPORTED_OBJECT_TYPES:
				raise ValueError(f"Unsupported object type: {obj.type}")
			for child in obj.children:
				validate(child)
		for obj in layer.objects:
			validate(obj)
		layers.append(layer.to_dict())
	if not layers:
		raise ValueError("No layers found")
	doc = new_document()
	doc["version"] = int(raw.get("version", 1))
	doc["target"] = target
	doc["canvas"] = canvas
	doc["optimize"] = {**doc["optimize"], **raw.get("optimize", {})}
	doc["layers"] = layers
	return doc


def new_document(width: int = CANVAS_WIDTH, height: int = CANVAS_HEIGHT) -> Dict[str, Any]:
	return {
		"format": "pdvector",
		"version": 1,
		"target": {
			"sdk": "3.1.1",
			"width": int(width),
			"height": int(height),
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
		if d.get("anchor", "screen") == "object":
			origin = style.get("_dither_origin", (0, 0))
			x -= int(origin[0])
			y -= int(origin[1])

		if dtype == "none":
			return True
		if level <= 0.0:
			return False
		if level >= 1.0:
			return True

		if dtype == "checker":
			return level > ((x + px + y + py) & 1)
		if dtype in ("line", "screen"):
			return level > ((y + py) & 1)
		matrices = {
			"bayer2x2": ((0, 2), (3, 1)),
			"bayer4x4": ((0, 8, 2, 10), (12, 4, 14, 6), (3, 11, 1, 9), (15, 7, 13, 5)),
			"bayer8x8": (
				(0, 32, 8, 40, 2, 34, 10, 42), (48, 16, 56, 24, 50, 18, 58, 26),
				(12, 44, 4, 36, 14, 46, 6, 38), (60, 28, 52, 20, 62, 30, 54, 22),
				(3, 35, 11, 43, 1, 33, 9, 41), (51, 19, 59, 27, 49, 17, 57, 25),
				(15, 47, 7, 39, 13, 45, 5, 37), (63, 31, 55, 23, 61, 29, 53, 21),
			),
		}
		matrix = matrices.get(dtype)
		if matrix:
			size = len(matrix)
			threshold = (matrix[(y + py) % size][(x + px) % size] + 0.5) / (size * size)
			return level > threshold
		if dtype == "custom":
			pattern = d.get("pattern", [])
			if len(pattern) == 8 and all(isinstance(row, list) and len(row) == 8 for row in pattern):
				return level > float(pattern[(y + py) % 8][(x + px) % 8])
		return True

	def _stamp(self, cx: int, cy: int, width: int, cap: str, style: Dict[str, Any]) -> None:
		radius = max(0.5, width / 2.0)
		iradius = max(0, int(math.ceil(radius)))
		for oy in range(-iradius, iradius + 1):
			for ox in range(-iradius, iradius + 1):
				if ox * ox + oy * oy <= radius * radius + 0.25:
					x, y = cx + ox, cy + oy
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

		# A pixel-center distance test gives stable diagonals and applies the
		# requested line cap instead of treating every cap as a round stamp.
		half = max(0.5, width / 2.0)
		length = math.hypot(dx, dy)
		ux, uy = dx / length, dy / length
		extend = half if cap == "square" else 0.0
		min_x = math.floor(min(x0, x1) - half - extend)
		max_x = math.ceil(max(x0, x1) + half + extend)
		min_y = math.floor(min(y0, y1) - half - extend)
		max_y = math.ceil(max(y0, y1) + half + extend)
		for y in range(min_y, max_y + 1):
			for x in range(min_x, max_x + 1):
				vx, vy = x - x0, y - y0
				t = (vx * ux + vy * uy) / length
				if cap == "square":
					t = max(-extend / length, min(1.0 + extend / length, t))
				elif cap == "butt" and (t < 0.0 or t > 1.0):
					continue
				elif cap == "round":
					t = max(0.0, min(1.0, t))
				qx = x0 + t * dx
				qy = y0 + t * dy
				if (x - qx) ** 2 + (y - qy) ** 2 <= half * half + 0.25:
					if self._dither_accept(x, y, style):
						self._apply_blend(x, y, style)

	def fill_polygon(self, points: List[IntPoint], style: Dict[str, Any]) -> None:
		for x0, x1, y in polygon_scanlines(points, self.width, self.height):
			for x in range(x0, x1 + 1):
				if self._dither_accept(x, y, style):
					self._apply_blend(x, y, style)

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
		self.selected_object_index: Optional[int] = None
		self.current_points: List[Point] = []
		self.drag_endpoint: Optional[int] = None
		self.drag_history_started = False
		self.editor_scale = EDITOR_SCALE
		self.canvas_width = CANVAS_WIDTH
		self.canvas_height = CANVAS_HEIGHT
		# Keep the editor viewport stable.  The canvas content may become larger
		# or smaller when zooming, but the surrounding window must not follow its
		# requested size.
		self.editor_viewport_width = self.canvas_width * self.editor_scale
		self.editor_viewport_height = self.canvas_height * self.editor_scale
		self.undo_stack: List[Dict[str, Any]] = []
		self.redo_stack: List[Dict[str, Any]] = []
		self.status_text = tk.StringVar(value="Ready")

		self.tool_var = tk.StringVar(value="line")
		self.rounding_var = tk.StringVar(value="nearest")
		self.mode_var = tk.StringVar(value="readable")
		self.output_type_var = tk.StringVar(value="line-only")
		self.color_var = tk.StringVar(value="black")
		self.blend_var = tk.StringVar(value="normal")
		self.cap_var = tk.StringVar(value="butt")
		self.width_var = tk.IntVar(value=1)
		self.fill_var = tk.BooleanVar(value=False)
		self.dither_type_var = tk.StringVar(value="none")
		self.dither_level_var = tk.DoubleVar(value=1.0)
		self.dither_anchor_var = tk.StringVar(value="screen")
		self.layer_visible_var = tk.BooleanVar(value=True)
		self.mode_edit_var = tk.StringVar(value="stroke")
		self.resolution_width_var = tk.IntVar(value=CANVAS_WIDTH)
		self.resolution_height_var = tk.IntVar(value=CANVAS_HEIGHT)

		self.rasterizer = Rasterizer(self.canvas_width, self.canvas_height)
		self.preview_image = tk.PhotoImage(width=self.canvas_width, height=self.canvas_height)
		self.preview_zoom = 1

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
			values=["pixel", "line", "polyline", "polygon", "rect", "ellipse", "bezier", "path"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(0, 8))
		tk.Label(left, text="Edit mode").pack(anchor=tk.W)
		mode_buttons = ttk.Frame(left)
		mode_buttons.pack(anchor=tk.W, pady=(0, 8))
		tk.Radiobutton(mode_buttons, text="Stroke", variable=self.mode_edit_var, value="stroke", command=lambda: self.set_edit_mode("stroke")).pack(side=tk.LEFT)
		tk.Radiobutton(mode_buttons, text="Move", variable=self.mode_edit_var, value="move", command=lambda: self.set_edit_mode("move")).pack(side=tk.LEFT)

		tk.Label(left, text="Resolution").pack(anchor=tk.W)
		resolution = ttk.Frame(left)
		resolution.pack(anchor=tk.W, pady=(0, 8))
		tk.Spinbox(resolution, from_=1, to=4096, textvariable=self.resolution_width_var, width=6).pack(side=tk.LEFT)
		tk.Label(resolution, text=" x ").pack(side=tk.LEFT)
		tk.Spinbox(resolution, from_=1, to=4096, textvariable=self.resolution_height_var, width=6).pack(side=tk.LEFT)
		tk.Button(resolution, text="Apply", command=self.apply_resolution).pack(side=tk.LEFT, padx=(4, 0))

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
		tk.Spinbox(left, from_=1, to=8, textvariable=self.width_var, width=6).pack(anchor=tk.W, pady=(2, 8))
		tk.Checkbutton(left, text="Fill closed shape", variable=self.fill_var).pack(anchor=tk.W)
		tk.Label(left, text="Dither").pack(anchor=tk.W, pady=(4, 0))
		ttk.Combobox(
			left,
			textvariable=self.dither_type_var,
			values=["none", "checker", "line", "screen", "bayer2x2", "bayer4x4", "bayer8x8", "custom"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W)
		tk.Spinbox(left, from_=0.0, to=1.0, increment=0.05, textvariable=self.dither_level_var, width=6).pack(anchor=tk.W, pady=(2, 8))
		ttk.Combobox(
			left,
			textvariable=self.dither_anchor_var,
			values=["screen", "object"],
			state="readonly",
			width=14,
		).pack(anchor=tk.W, pady=(0, 8))

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

		ttk.Label(left, text="Vectors").pack(anchor=tk.W)
		self.object_list = tk.Listbox(left, height=8, width=24, exportselection=False)
		self.object_list.pack(anchor=tk.W)
		self.object_list.bind("<<ListboxSelect>>", self.on_object_select)
		object_btns = ttk.Frame(left)
		object_btns.pack(anchor=tk.W, pady=(4, 8))
		tk.Button(object_btns, text="Show/Hide", command=self.toggle_selected_object).pack(side=tk.LEFT)
		tk.Button(object_btns, text="Duplicate", command=self.duplicate_selected_object).pack(side=tk.LEFT, padx=(4, 0))
		tk.Button(object_btns, text="Delete", command=self.delete_selected_object).pack(side=tk.LEFT, padx=(4, 0))

		tk.Label(left, text="Export").pack(anchor=tk.W)
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
		tk.Button(action_btns, text="Undo", command=self.undo).pack(side=tk.LEFT)
		tk.Button(action_btns, text="Redo", command=self.redo).pack(side=tk.LEFT, padx=(4, 0))
		ttk.Button(action_btns, text="Export Lua", command=self.on_export_lua).pack(side=tk.LEFT)
		ttk.Button(action_btns, text="Optimize", command=self.on_optimize_preview).pack(side=tk.LEFT, padx=(4, 0))

		tk.Button(action_btns, text="Copy Lua", command=self.copy_lua_to_clipboard).pack(side=tk.LEFT, padx=(4, 0))
		tk.Button(action_btns, text="Copy JSON", command=self.copy_json_to_clipboard).pack(side=tk.LEFT, padx=(4, 0))

		self.editor_viewport = ttk.Frame(
			center,
			width=self.editor_viewport_width,
			height=self.editor_viewport_height,
		)
		self.editor_viewport.pack_propagate(False)
		self.editor_viewport.pack(anchor=tk.NW)
		self.editor_canvas = self._create_editor_canvas()
		self.editor_canvas.pack(fill=tk.BOTH, expand=True)
		self.root.bind("<Return>", self.on_finalize_key)
		self.root.bind("<Escape>", self.on_cancel_key)
		self.root.bind("<Control-z>", lambda _event: self.undo())
		self.root.bind("<Control-y>", lambda _event: self.redo())
		self.root.bind("<Command-z>", lambda _event: self.undo())
		self.root.bind("<Command-y>", lambda _event: self.redo())

		ttk.Label(right, text="1-bit Preview").pack(anchor=tk.W)
		self.preview_label = ttk.Label(right, image=self.preview_image)
		self.preview_label.pack(anchor=tk.NW)
		ttk.Separator(right, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=6)
		ttk.Label(right, text="Status").pack(anchor=tk.W)
		ttk.Label(right, textvariable=self.status_text, wraplength=360, justify=tk.LEFT).pack(anchor=tk.W)

	def _create_editor_canvas(self) -> tk.Canvas:
		canvas = tk.Canvas(
			self.editor_viewport,
			width=self.editor_viewport_width,
			height=self.editor_viewport_height,
			bg="white",
			highlightthickness=1,
			highlightbackground="#333",
		)
		canvas.bind("<Button-1>", self.on_canvas_left_click)
		canvas.bind("<ButtonRelease-1>", self.on_canvas_left_release)
		canvas.bind("<Button-3>", self.on_canvas_right_click)
		canvas.bind("<Motion>", self.on_canvas_motion)
		canvas.bind("<MouseWheel>", self.on_canvas_wheel)
		canvas.bind("<Button-4>", lambda _event: self.adjust_editor_zoom(1))
		canvas.bind("<Button-5>", lambda _event: self.adjust_editor_zoom(-1))
		return canvas

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

	def _canvas_size(self) -> Tuple[int, int]:
		return (
			int(getattr(self, "canvas_width", self.doc.get("target", {}).get("width", CANVAS_WIDTH))),
			int(getattr(self, "canvas_height", self.doc.get("target", {}).get("height", CANVAS_HEIGHT))),
		)

	def _record_history(self) -> None:
		self.undo_stack.append(copy.deepcopy(self.doc))
		self.redo_stack.clear()
		if len(self.undo_stack) > 100:
			self.undo_stack.pop(0)

	def undo(self) -> None:
		if not self.undo_stack:
			self.set_status("Nothing to undo.")
			return
		self.redo_stack.append(copy.deepcopy(self.doc))
		self.doc = self.undo_stack.pop()
		self._sync_document_view()
		self.set_status("Undo")

	def redo(self) -> None:
		if not self.redo_stack:
			self.set_status("Nothing to redo.")
			return
		self.undo_stack.append(copy.deepcopy(self.doc))
		self.doc = self.redo_stack.pop()
		self._sync_document_view()
		self.set_status("Redo")

	def _sync_document_view(self) -> None:
		self.canvas_width = int(self.doc.get("target", {}).get("width", CANVAS_WIDTH))
		self.canvas_height = int(self.doc.get("target", {}).get("height", CANVAS_HEIGHT))
		self.resolution_width_var.set(self.canvas_width)
		self.resolution_height_var.set(self.canvas_height)
		self.rounding_var.set(self.doc.get("target", {}).get("rounding", "nearest"))
		self.dither_anchor_var.set(self.doc.get("canvas", {}).get("ditherAnchor", "screen"))
		self.rasterizer = Rasterizer(self.canvas_width, self.canvas_height)
		self.preview_image = tk.PhotoImage(width=self.canvas_width, height=self.canvas_height)
		self.refresh_layer_list()
		self.refresh_object_list()
		self.redraw_all()

	def apply_resolution(self) -> None:
		width = max(1, int(self.resolution_width_var.get()))
		height = max(1, int(self.resolution_height_var.get()))
		if (width, height) == (self.canvas_width, self.canvas_height):
			return
		self._record_history()
		self.doc.setdefault("target", {})["width"] = width
		self.doc["target"]["height"] = height
		self._sync_document_view()
		self.set_status(f"Resolution: {width} x {height}")

	def _selected_object(self) -> Optional[VectorObject]:
		if self.selected_object_index is None:
			return None
		layers = self.get_layers()
		if not layers or not (0 <= self.current_layer_index < len(layers)):
			return None
		objects = layers[self.current_layer_index].objects
		if not (0 <= self.selected_object_index < len(objects)):
			return None
		return objects[self.selected_object_index]

	def set_edit_mode(self, mode: str) -> None:
		self.mode_edit_var.set(mode)
		self.current_points.clear()
		self.drag_endpoint = None
		self.set_status("Stroke mode" if mode == "stroke" else "Move mode")

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

	def refresh_object_list(self) -> None:
		if not hasattr(self, "object_list"):
			return
		self.object_list.delete(0, tk.END)
		layers = self.get_layers()
		if not layers:
			return
		for index, obj in enumerate(layers[self.current_layer_index].objects):
			marker = "[x]" if obj.visible else "[ ]"
			self.object_list.insert(tk.END, f"{marker} {index + 1}: {obj.type}")
		if self.selected_object_index is not None and self.selected_object_index < len(layers[self.current_layer_index].objects):
			self.object_list.selection_set(self.selected_object_index)

	def on_object_select(self, _event: Any) -> None:
		sel = self.object_list.curselection()
		self.selected_object_index = sel[0] if sel else None
		self.redraw_all()

	def _mutate_selected_object(self, action: str) -> None:
		if self.selected_object_index is None:
			return
		layers = self.get_layers()
		objects = layers[self.current_layer_index].objects
		if not (0 <= self.selected_object_index < len(objects)):
			return
		self._record_history()
		obj = objects[self.selected_object_index]
		if action == "toggle":
			obj.visible = not obj.visible
		elif action == "delete":
			del objects[self.selected_object_index]
			self.selected_object_index = min(self.selected_object_index, len(objects) - 1) if objects else None
		elif action == "duplicate":
			objects.insert(self.selected_object_index + 1, copy.deepcopy(obj))
			self.selected_object_index += 1
		self.set_layers(layers)
		self.refresh_object_list()
		self.redraw_all()

	def toggle_selected_object(self) -> None:
		self._mutate_selected_object("toggle")

	def delete_selected_object(self) -> None:
		self._mutate_selected_object("delete")

	def duplicate_selected_object(self) -> None:
		self._mutate_selected_object("duplicate")

	def add_layer(self) -> None:
		self._record_history()
		layers = self.get_layers()
		new_id = f"layer{len(layers) + 1}"
		layers.append(Layer(id=new_id, visible=True, objects=[]))
		self.set_layers(layers)
		self.current_layer_index = len(layers) - 1
		self.refresh_layer_list()
		self.selected_object_index = None
		self.refresh_object_list()
		self.redraw_all()

	def delete_layer(self) -> None:
		layers = self.get_layers()
		if len(layers) <= 1:
			messagebox.showinfo("DotStroke", "At least one layer is required.")
			return
		self._record_history()
		del layers[self.current_layer_index]
		self.set_layers(layers)
		self.current_layer_index = clamp(self.current_layer_index, 0, len(layers) - 1)
		self.refresh_layer_list()
		self.selected_object_index = None
		self.refresh_object_list()
		self.redraw_all()

	def on_layer_select(self, _event: Any) -> None:
		sel = self.layer_list.curselection()
		if not sel:
			return
		self.current_layer_index = sel[0]
		self.selected_object_index = None
		layers = self.get_layers()
		self.layer_visible_var.set(layers[self.current_layer_index].visible)
		self.refresh_object_list()
		self.redraw_all()

	def on_layer_visible_toggle(self) -> None:
		layers = self.get_layers()
		if not layers:
			return
		self._record_history()
		layers[self.current_layer_index].visible = bool(self.layer_visible_var.get())
		self.set_layers(layers)
		self.refresh_layer_list()
		self.refresh_object_list()
		self.redraw_all()

	def _editor_to_doc_point(self, event: tk.Event) -> Point:
		return (event.x / self.editor_scale, event.y / self.editor_scale)

	def _active_style(self) -> Dict[str, Any]:
		style = default_style()
		style["color"] = self.color_var.get()
		style["blend"] = self.blend_var.get()
		style["cap"] = self.cap_var.get()
		style["width"] = max(1, int(self.width_var.get()))
		style["fill"] = bool(self.fill_var.get())
		style["dither"] = {
			"type": self.dither_type_var.get(),
			"level": max(0.0, min(1.0, float(self.dither_level_var.get()))),
			"phase": [0, 0],
			"anchor": self.dither_anchor_var.get(),
		}
		if style["blend"] == "xor" and style["dither"]["type"] != "none":
			style["dither"] = {**style["dither"], "type": "none", "level": 1.0}
		return style

	def on_canvas_left_click(self, event: tk.Event) -> None:
		p = self._editor_to_doc_point(event)
		rounded = snap_point(p, self.rounding_var.get())
		if self.mode_edit_var.get() == "move":
			obj = self._selected_object()
			if obj and obj.points:
				candidates = [0] if len(obj.points) == 1 else [0, len(obj.points) - 1]
				display_points = self._object_to_poly_points(obj.points, obj.transform)
				endpoint = min(candidates, key=lambda i: math.hypot(display_points[i][0] - p[0], display_points[i][1] - p[1]))
				if math.hypot(display_points[endpoint][0] - p[0], display_points[endpoint][1] - p[1]) <= max(6.0, 8.0 / self.editor_scale):
					self._record_history()
					self.drag_endpoint = endpoint
					self.drag_history_started = True
					self.set_status("Dragging vector endpoint")
				return
			self.set_status("Select a vector and grab its endpoint in Move mode.")
			return
		tool = self.tool_var.get()

		if tool == "pixel":
			self._commit_object("pixel", [rounded], False)
		elif tool == "line":
			self.current_points.append(rounded)
			if len(self.current_points) == 2:
				self._commit_object("line", self.current_points[:], False)
				self.current_points.clear()
		elif tool in ("polyline", "polygon", "rect", "ellipse", "bezier", "path"):
			self.current_points.append(rounded)
		self.redraw_all()

	def on_canvas_left_release(self, _event: tk.Event) -> None:
		if self.drag_endpoint is not None:
			self.drag_endpoint = None
			self.drag_history_started = False
			self.set_status("Endpoint moved")

	def adjust_editor_zoom(self, direction: int) -> None:
		old_scale = self.editor_scale
		self.editor_scale = max(1, min(8, self.editor_scale + direction))
		if self.editor_scale == old_scale:
			return

		# Zoom the existing items in place.  Rebuilding and swapping canvases
		# here can still expose a blank frame on some Tk/macOS combinations,
		# especially for very small documents such as 32x32.
		ratio = self.editor_scale / old_scale
		self.editor_canvas.scale("all", 0, 0, ratio, ratio)
		for item in self.editor_canvas.find_withtag("editor-object"):
			width = float(self.editor_canvas.itemcget(item, "width") or 1)
			self.editor_canvas.itemconfigure(item, width=max(1, round(width * ratio)))
		self.set_status(f"Editor zoom: {self.editor_scale}x")

	def on_canvas_wheel(self, event: tk.Event) -> None:
		self.adjust_editor_zoom(1 if event.delta > 0 else -1)

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
		if self.drag_endpoint is not None and self.mode_edit_var.get() == "move":
			layers = self.get_layers()
			if layers and self.selected_object_index is not None:
				obj = layers[self.current_layer_index].objects[self.selected_object_index]
				if obj.points:
					obj.points[self.drag_endpoint] = inverse_transform_point(snap_point(p, self.rounding_var.get()), obj.transform)
					self.set_layers(layers)
					self.redraw_all()
		px = int(clamp(round(p[0]), 0, self.canvas_width - 1))
		py = int(clamp(round(p[1]), 0, self.canvas_height - 1))
		self.set_status(
			f"Cursor: ({px}, {py}) | Tool: {self.tool_var.get()} | Pending points: {len(self.current_points)}"
		)

	def finalize_current_shape(self) -> None:
		tool = self.tool_var.get()
		if tool in ("polyline", "path") and len(self.current_points) >= 2:
			self._commit_object(tool, self.current_points[:], False)
			self.current_points.clear()
		elif tool == "polygon" and len(self.current_points) >= 3:
			self._commit_object("polygon", self.current_points[:], True)
			self.current_points.clear()
		elif tool in ("rect", "ellipse") and len(self.current_points) >= 2:
			self._commit_object(tool, self.current_points[:2], True)
			self.current_points.clear()
		elif tool == "bezier" and len(self.current_points) >= 3:
			self._commit_object("bezier", self.current_points[:4], False)
			self.current_points.clear()
		else:
			self.set_status("Not enough points to finalize shape.")
		self.redraw_all()

	def _commit_object(self, obj_type: str, points: List[Point], closed: bool) -> None:
		layers = self.get_layers()
		layer = layers[self.current_layer_index]
		if self.doc.get("optimize", {}).get("removeDuplicatePoints", True):
			points = remove_duplicate_points(points)
		if obj_type == "pixel":
			if not points:
				return
		elif len(points) < 2:
			self.set_status("Object ignored: too few points.")
			return
		if obj_type == "polygon" and len(points) < 3:
			self.set_status("Polygon ignored: at least 3 points required.")
			return
		self._record_history()

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
		self.selected_object_index = len(layer.objects) - 1
		self.set_status(f"Added {obj_type} with {len(points)} points.")
		self.refresh_layer_list()
		self.refresh_object_list()

	def _object_to_poly_points(
		self,
		points: List[Point],
		transform: Optional[Dict[str, Any]] = None,
		parent_transform: Optional[Dict[str, Any]] = None,
	) -> List[Point]:
		snap_mode = self.rounding_var.get() or self.doc.get("target", {}).get("rounding", "nearest")
		transform = transform or {}
		parent_transform = parent_transform or {}
		result: List[Point] = []
		for p in points:
			tp = transform_point(p, transform)
			if parent_transform:
				tp = apply_transform(tp, parent_transform)
			sp = snap_point(tp, snap_mode)
			result.append(sp if snap_mode == "subpixel" else (int(sp[0]), int(sp[1])))
		return result

	def _shape_points(self, obj: VectorObject) -> List[Point]:
		if obj.type == "rect" and len(obj.points) == 2:
			(x0, y0), (x1, y1) = obj.points
			return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
		if obj.type == "ellipse" and len(obj.points) == 2:
			(x0, y0), (x1, y1) = obj.points
			cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
			rx, ry = abs(x1 - x0) / 2.0, abs(y1 - y0) / 2.0
			return [
				(cx + rx * math.cos(2 * math.pi * i / 32), cy + ry * math.sin(2 * math.pi * i / 32))
				for i in range(32)
			]
		if obj.type == "bezier" and len(obj.points) in (3, 4):
			p = obj.points
			out: List[Point] = []
			for i in range(17):
				t = i / 16.0
				if len(p) == 3:
					x = (1 - t) ** 2 * p[0][0] + 2 * (1 - t) * t * p[1][0] + t ** 2 * p[2][0]
					y = (1 - t) ** 2 * p[0][1] + 2 * (1 - t) * t * p[1][1] + t ** 2 * p[2][1]
				else:
					x = (1 - t) ** 3 * p[0][0] + 3 * (1 - t) ** 2 * t * p[1][0] + 3 * (1 - t) * t ** 2 * p[2][0] + t ** 3 * p[3][0]
					y = (1 - t) ** 3 * p[0][1] + 3 * (1 - t) ** 2 * t * p[1][1] + 3 * (1 - t) * t ** 2 * p[2][1] + t ** 3 * p[3][1]
				out.append((x, y))
			return out
		return obj.points

	def _style_for_object(self, obj: VectorObject, points: List[IntPoint]) -> Dict[str, Any]:
		style = copy_style(obj.style)
		if style.get("blend") == "xor" and style.get("dither", {}).get("type", "none") != "none":
			# Playdate does not support setColor(XOR) and setDitherPattern together.
			style["dither"] = {**style.get("dither", {}), "type": "none", "level": 1.0}
		if points:
			style["_dither_origin"] = points[0]
		return style

	def _build_commands(self, output_type: str) -> List[Dict[str, Any]]:
		commands: List[Dict[str, Any]] = []
		canvas_width, canvas_height = self._canvas_size()

		def visit(obj: VectorObject, parent_transform: Optional[Dict[str, Any]] = None) -> None:
			parent_transform = parent_transform or {}
			local_points = self._shape_points(obj)
			points = self._object_to_poly_points(local_points, obj.transform, parent_transform)
			if self.doc.get("optimize", {}).get("removeDuplicatePoints", True):
				points = remove_duplicate_points(points)
			tolerance = float(self.doc.get("optimize", {}).get("simplifyTolerance", 0) or 0)
			if tolerance > 0 and obj.type not in ("pixel", "group"):
				points = simplify_points(points, tolerance)
			style = self._style_for_object(obj, points)
			if obj.type == "group":
				for child in obj.children:
					if child.visible:
						visit(child, obj.transform if not parent_transform else compose_transforms(parent_transform, obj.transform))
				return
			if points and self.doc.get("target", {}).get("clip", True):
				bbox = (min(p[0] for p in points), min(p[1] for p in points), max(p[0] for p in points), max(p[1] for p in points))
				if bbox_outside_screen(bbox, canvas_width, canvas_height):
					return
			if obj.type == "pixel" and points:
				commands.append({"kind": "pixel", "points": [points[0]], "style": style})
				return
			if len(points) < 2:
				return
			closed = obj.closed or obj.type in ("polygon", "rect", "ellipse")
			is_fill = bool(style.get("fill", False)) and obj.type in ("polygon", "rect", "ellipse")
			rotated_native_shape = obj.type in ("rect", "ellipse") and abs(float(obj.transform.get("rotation", 0))) > 1e-9
			if output_type == "sdk-native" and obj.type in ("polygon", "rect", "ellipse") and not rotated_native_shape:
				commands.append({"kind": obj.type, "points": points, "closed": closed, "fill": is_fill, "style": style})
			else:
				commands.append({"kind": "polyline", "points": points, "closed": closed, "style": style, "fill": is_fill})

		for layer in self.get_layers():
			if layer.visible:
				for obj in layer.objects:
					if obj.visible:
						visit(obj)
		return commands

	def _build_segments(
		self,
		optimize: bool,
		output_type: str,
		rasterize: bool = True,
	) -> List[Segment]:
		segments: List[Segment] = []
		canvas_width, canvas_height = self._canvas_size()
		for command in self._build_commands(output_type):
			kind = command["kind"]
			raw_pts: List[Point] = command["points"]
			pts: List[Point] = raw_pts if not rasterize else [
				(int(snap_value(p[0], "nearest")), int(snap_value(p[1], "nearest"))) for p in raw_pts
			]
			raster_pts: List[IntPoint] = [
				(int(snap_value(p[0], "nearest")), int(snap_value(p[1], "nearest"))) for p in raw_pts
			]
			st = command["style"]
			if kind == "pixel":
				p = pts[0]
				segments.append((p[0], p[1], p[0], p[1], st))
				continue
			if command.get("fill") and len(raster_pts) >= 3:
				for x0, x1, y in polygon_scanlines(raster_pts, canvas_width, canvas_height):
					segments.append((x0, y, x1, y, st))
			for i in range(len(pts) - 1):
				segments.append((pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1], st))
			if command.get("closed") and len(pts) >= 3:
				segments.append((pts[-1][0], pts[-1][1], pts[0][0], pts[0][1], st))

		if self.doc.get("target", {}).get("clip", True):
			segments = [seg for seg in segments if not bbox_outside_screen(line_bbox(seg[:4]), canvas_width, canvas_height)]

		if optimize and self.doc.get("optimize", {}).get("mergeCollinearLines", True):
			segments = merge_collinear_segments(segments)

		return segments

	def redraw_all(self) -> None:
		self._redraw_editor()
		self._render_preview()

	def _redraw_editor(self) -> None:
		# Zooming uses in-place item scaling, so this full redraw is only used
		# when the document itself changes.
		self.editor_canvas.delete("all")
		self._draw_editor_grid()
		self._draw_editor_objects()
		self._draw_pending_shape()

	def _draw_editor_grid(self) -> None:
		for x in range(0, self.canvas_width + 1, 20):
			sx = x * self.editor_scale
			self.editor_canvas.create_line(
				sx, 0, sx, self.canvas_height * self.editor_scale,
				fill="#efefef", tags=("editor-grid",),
			)
		for y in range(0, self.canvas_height + 1, 20):
			sy = y * self.editor_scale
			self.editor_canvas.create_line(
				0, sy, self.canvas_width * self.editor_scale, sy,
				fill="#efefef", tags=("editor-grid",),
			)

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
				x1 * self.editor_scale,
				y1 * self.editor_scale,
				x2 * self.editor_scale,
				y2 * self.editor_scale,
				fill=draw_color,
				width=max(1, int(style.get("width", 1)) * self.editor_scale),
				tags=("editor-object",),
			)
		obj = self._selected_object()
		if obj and obj.points:
			handle_points = self._object_to_poly_points(obj.points, obj.transform)
			indices = [0] if len(handle_points) == 1 else [0, len(handle_points) - 1]
			for index in indices:
				x, y = handle_points[index]
				radius = max(3, self.editor_scale * 2)
				self.editor_canvas.create_oval(
					x * self.editor_scale - radius,
					y * self.editor_scale - radius,
					x * self.editor_scale + radius,
					y * self.editor_scale + radius,
					outline="#ff0066",
					width=1,
					tags=("editor-handle",),
				)

	def _draw_pending_shape(self) -> None:
		if not self.current_points:
			return
		pts = [(p[0] * self.editor_scale, p[1] * self.editor_scale) for p in self.current_points]
		if len(pts) == 1:
			x, y = pts[0]
			self.editor_canvas.create_oval(
				x - 2, y - 2, x + 2, y + 2,
				fill="#333", outline="", tags=("editor-pending",),
			)
			return
		flat: List[float] = []
		for p in pts:
			flat.extend([p[0], p[1]])
		self.editor_canvas.create_line(*flat, fill="#f06", dash=(3, 2), tags=("editor-pending",))

	def _render_preview(self) -> None:
		self.rasterizer.clear()
		segments = self._build_segments(optimize=True, output_type=self.output_type_var.get())
		self.rasterizer.draw_segments(segments)

		# Tk PhotoImage.put expects rows of explicit color tokens, not run-length tuples.
		rows: List[str] = []
		for y in range(self.canvas_height):
			row_colors = ["#000000" if self.rasterizer.pixels[y][x] else "#ffffff" for x in range(self.canvas_width)]
			rows.append("{" + " ".join(row_colors) + "}")
		self.preview_image.put(" ".join(rows), to=(0, 0, self.canvas_width, self.canvas_height))
		zoomed = self.preview_image.zoom(self.preview_zoom, self.preview_zoom)
		self.preview_label.configure(image=zoomed)
		self.preview_label.image = zoomed

	def on_new(self) -> None:
		if messagebox.askyesno("DotStroke", "Create a new document?"):
			self._record_history()
			self.doc = new_document()
			self.current_layer_index = 0
			self.selected_object_index = None
			self.current_points.clear()
			self.rounding_var.set("nearest")
			self.dither_anchor_var.set("screen")
			self.dither_type_var.set("none")
			self.dither_level_var.set(1.0)
			self._sync_document_view()
			self.set_status("New document created.")

	def on_save_json(self) -> None:
		self.doc["target"]["rounding"] = self.rounding_var.get()
		self.doc.setdefault("canvas", {})["ditherAnchor"] = self.dither_anchor_var.get()
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
				normalized = normalize_document(doc)
				self._record_history()
				self.doc = normalized
			self.current_layer_index = 0
			self.selected_object_index = None
			self.rounding_var.set(self.doc.get("target", {}).get("rounding", "nearest"))
			self.dither_anchor_var.set(self.doc.get("canvas", {}).get("ditherAnchor", "screen"))
			self.dither_type_var.set("none")
			self.dither_level_var.set(1.0)
			self.current_points.clear()
			self._sync_document_view()
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

	def _lua_dither_lines(self, style: Dict[str, Any], color_expr: str = "currentColor", indent: str = "    ") -> List[str]:
		d = style.get("dither", {})
		dtype = d.get("type", "none")
		level = max(0.0, min(1.0, float(d.get("level", 1.0))))
		if dtype == "none" or level >= 1.0:
			return [f"{indent}-- clear dither state", f"{indent}gfx.setColor({color_expr})"]
		if style.get("blend") == "xor":
			return [
				f"{indent}-- xor + dither is unsupported; emit solid XOR",
				f"{indent}gfx.setColor({color_expr})",
			]
		mapping = {
			"checker": "gfx.image.kDitherTypeBayer2x2",
			"line": "gfx.image.kDitherTypeHorizontalLine",
			"screen": "gfx.image.kDitherTypeScreen",
			"bayer2x2": "gfx.image.kDitherTypeBayer2x2",
			"bayer4x4": "gfx.image.kDitherTypeBayer4x4",
			"bayer8x8": "gfx.image.kDitherTypeBayer8x8",
		}
		dither_expr = mapping.get(dtype, "gfx.image.kDitherTypeBayer8x8")
		return [f"{indent}gfx.setDitherPattern({level:.3f}, {dither_expr})"]

	def _lua_set_style(self, style: Dict[str, Any], indent: str = "    ", color_expr: Optional[str] = None) -> List[str]:
		color_expr = color_expr or self._lua_color_expr(style)
		lines = [
			f"{indent}gfx.setColor({color_expr})",
			f"{indent}gfx.setLineWidth({int(style.get('width', 1))})",
			f"{indent}gfx.setLineCapStyle({self._lua_cap_expr(style.get('cap', 'butt'))})",
		]
		lines.extend(self._lua_dither_lines(style, color_expr, indent))
		return lines

	def _lua_points_expr(self, points: List[IntPoint]) -> str:
		values: List[str] = []
		for px, py in points:
			values.extend([f"x + {px}", f"y + {py}"])
		return ", ".join(values)

	def _lua_draw_command(self, command: Dict[str, Any], indent: str = "    ") -> List[str]:
		kind = command["kind"]
		pts: List[IntPoint] = command["points"]
		lines: List[str] = []
		if kind == "pixel":
			lines.append(f"{indent}gfx.drawPixel({self._lua_points_expr(pts[:1])})")
		elif kind in ("polyline", "line"):
			for a, b in zip(pts, pts[1:]):
				lines.append(f"{indent}gfx.drawLine(x + {a[0]}, y + {a[1]}, x + {b[0]}, y + {b[1]})")
			if command.get("closed") and len(pts) >= 3:
				a, b = pts[-1], pts[0]
				lines.append(f"{indent}gfx.drawLine(x + {a[0]}, y + {a[1]}, x + {b[0]}, y + {b[1]})")
		elif kind == "polygon":
			call = "gfx.fillPolygon" if command.get("fill") else "gfx.drawPolygon"
			lines.append(f"{indent}{call}({self._lua_points_expr(pts)})")
		elif kind == "rect":
			xs, ys = [p[0] for p in pts], [p[1] for p in pts]
			x0, y0, x1, y1 = min(xs), min(ys), max(ys), max(ys)
			call = "gfx.fillRect" if command.get("fill") else "gfx.drawRect"
			lines.append(f"{indent}{call}(x + {x0}, y + {y0}, {x1 - x0}, {y1 - y0})")
		elif kind == "ellipse":
			xs, ys = [p[0] for p in pts], [p[1] for p in pts]
			x0, y0, x1, y1 = min(xs), min(ys), max(xs), max(ys)
			call = "gfx.fillEllipse" if command.get("fill") else "gfx.drawEllipse"
			lines.append(f"{indent}{call}(x + {x0}, y + {y0}, {x1 - x0}, {y1 - y0})")
		return lines

	def _generate_lua(self) -> str:
		mode = self.mode_var.get()
		output_type = self.output_type_var.get()
		segments = self._build_segments(optimize=True, output_type="line-only", rasterize=False)
		commands = self._build_commands(output_type)

		def lua_quote(value: str) -> str:
			return "'" + value.replace("\\", "\\\\").replace("'", "\\'") + "'"

		lines: List[str] = []
		lines.append("local gfx <const> = playdate.graphics")
		lines.append("")
		lines.append("local function resetGraphicsState()")
		lines.append("    gfx.setColor(gfx.kColorBlack)")
		lines.append("    gfx.setLineWidth(1)")
		lines.append("    gfx.setLineCapStyle(gfx.kLineCapStyleButt)")
		lines.append("    -- setColor above also clears any previous dither state")
		lines.append("end")
		lines.append("")

		if mode == "compact" and output_type == "line-only":
			style_keys: List[Tuple[Any, ...]] = []
			styles: List[Dict[str, Any]] = []
			for seg in segments:
				key = style_key(seg[4])
				if key not in style_keys:
					style_keys.append(key)
					styles.append(seg[4])
			lines.append("local styles = {")
			for st in styles:
				lines.append(
					f"    {{ color = {self._lua_color_expr(st)}, width = {int(st.get('width', 1))}, cap = {lua_quote(st.get('cap', 'butt'))}, dither = {lua_quote(st.get('dither', {}).get('type', 'none'))}, level = {float(st.get('dither', {}).get('level', 1.0)):.3f}, anchor = {lua_quote(st.get('dither', {}).get('anchor', 'screen'))} }},"
				)
			lines.append("}")
			lines.append("")
			lines.append("local segments = {")
			for seg in segments:
				idx = style_keys.index(style_key(seg[4])) + 1
				lines.append(f"    {{{seg[0]}, {seg[1]}, {seg[2]}, {seg[3]}, {idx}}},")
			lines.append("}")
			lines.append("")

		if mode == "module" or (mode == "compact" and output_type == "sdk-native"):
			lines.append("local iconData = {")
			lines.append("    commands = {")
			module_commands = commands if output_type == "sdk-native" else [
				{"kind": "line", "points": [(s[0], s[1]), (s[2], s[3])], "style": s[4]}
				for s in segments
			]
			for command in module_commands:
				st = command["style"]
				point_values = ", ".join(f"{p[0]}, {p[1]}" for p in command["points"])
				lines.append(
					f"        {{ kind={lua_quote(command['kind'])}, points={{ {point_values} }}, fill={str(bool(command.get('fill', False))).lower()}, closed={str(bool(command.get('closed', False))).lower()}, color={self._lua_color_expr(st)}, width={int(st.get('width', 1))}, cap={lua_quote(st.get('cap', 'butt'))}, dither={lua_quote(st.get('dither', {}).get('type', 'none'))}, level={float(st.get('dither', {}).get('level', 1.0)):.3f}, anchor={lua_quote(st.get('dither', {}).get('anchor', 'screen'))} }},"
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
			items = commands if output_type == "sdk-native" else [
				{"kind": "line", "points": [(s[0], s[1]), (s[2], s[3])], "style": s[4]}
				for s in segments
			]
			active_style: Optional[Tuple[Any, ...]] = None
			for command in items:
				command_style = style_key(command["style"])
				if command_style != active_style:
					lines.extend(self._lua_set_style(command["style"]))
					active_style = command_style
				lines.extend(self._lua_draw_command(command))
			lines.append("")

		elif mode == "compact" and output_type == "line-only":
			lines.append("    for _, seg in ipairs(segments) do")
			lines.append("        local st = styles[seg[5]]")
			lines.append("        gfx.setColor(st.color)")
			lines.append("        if st.dither == 'checker' or st.dither == 'bayer2x2' then gfx.setDitherPattern(st.level, gfx.image.kDitherTypeBayer2x2)")
			lines.append("        elseif st.dither == 'line' then gfx.setDitherPattern(st.level, gfx.image.kDitherTypeHorizontalLine)")
			lines.append("        elseif st.dither == 'screen' then gfx.setDitherPattern(st.level, gfx.image.kDitherTypeScreen)")
			lines.append("        elseif st.dither == 'bayer4x4' then gfx.setDitherPattern(st.level, gfx.image.kDitherTypeBayer4x4)")
			lines.append("        elseif st.dither == 'bayer8x8' then gfx.setDitherPattern(st.level, gfx.image.kDitherTypeBayer8x8)")
			lines.append("        else gfx.setColor(st.color) end")
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

		elif mode == "module" or (mode == "compact" and output_type == "sdk-native"):
			lines.append("    for _, s in ipairs(iconData.commands) do")
			lines.append("        gfx.setColor(s.color)")
			lines.append("        if s.dither == 'checker' or s.dither == 'bayer2x2' then gfx.setDitherPattern(s.level, gfx.image.kDitherTypeBayer2x2)")
			lines.append("        elseif s.dither == 'line' then gfx.setDitherPattern(s.level, gfx.image.kDitherTypeHorizontalLine)")
			lines.append("        elseif s.dither == 'screen' then gfx.setDitherPattern(s.level, gfx.image.kDitherTypeScreen)")
			lines.append("        elseif s.dither == 'bayer4x4' then gfx.setDitherPattern(s.level, gfx.image.kDitherTypeBayer4x4)")
			lines.append("        elseif s.dither == 'bayer8x8' then gfx.setDitherPattern(s.level, gfx.image.kDitherTypeBayer8x8)")
			lines.append("        else gfx.setColor(s.color) end")
			lines.append("        gfx.setLineWidth(s.width)")
			lines.append("        if s.cap == 'round' then gfx.setLineCapStyle(gfx.kLineCapStyleRound)")
			lines.append("        elseif s.cap == 'square' then gfx.setLineCapStyle(gfx.kLineCapStyleSquare)")
			lines.append("        else gfx.setLineCapStyle(gfx.kLineCapStyleButt) end")
			lines.append("        if s.kind == 'pixel' then")
			lines.append("            gfx.drawPixel(x + s.points[1], y + s.points[2])")
			lines.append("        elseif s.kind == 'polygon' then")
			lines.append("            local p = {}")
			lines.append("            for i = 1, #s.points do p[i] = (i % 2 == 1) and (x + s.points[i]) or (y + s.points[i]) end")
			lines.append("            if s.fill then gfx.fillPolygon(table.unpack(p)) else gfx.drawPolygon(table.unpack(p)) end")
			lines.append("        elseif s.kind == 'rect' or s.kind == 'ellipse' then")
			lines.append("            local minx, miny, maxx, maxy = s.points[1], s.points[2], s.points[1], s.points[2]")
			lines.append("            for i = 1, #s.points, 2 do minx = math.min(minx, s.points[i]); maxx = math.max(maxx, s.points[i]); miny = math.min(miny, s.points[i + 1]); maxy = math.max(maxy, s.points[i + 1]) end")
			lines.append("            local draw = s.kind == 'rect' and (s.fill and gfx.fillRect or gfx.drawRect) or (s.fill and gfx.fillEllipse or gfx.drawEllipse)")
			lines.append("            draw(x + minx, y + miny, maxx - minx, maxy - miny)")
			lines.append("        else")
			lines.append("            for i = 1, #s.points - 2, 2 do gfx.drawLine(x + s.points[i], y + s.points[i + 1], x + s.points[i + 2], y + s.points[i + 3]) end")
			lines.append("            if s.closed then gfx.drawLine(x + s.points[#s.points - 1], y + s.points[#s.points], x + s.points[1], y + s.points[2]) end")
			lines.append("        end")
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

	def copy_lua_to_clipboard(self) -> None:
		lua_code = self._generate_lua()
		self.root.clipboard_clear()
		self.root.clipboard_append(lua_code)
		self.root.update()
		self.set_status("Lua output copied to clipboard.")

	def copy_json_to_clipboard(self) -> None:
		self.doc["target"]["rounding"] = self.rounding_var.get()
		self.doc.setdefault("canvas", {})["ditherAnchor"] = self.dither_anchor_var.get()
		self.root.clipboard_clear()
		self.root.clipboard_append(json.dumps(self.doc, indent=2, ensure_ascii=False))
		self.root.update()
		self.set_status("JSON output copied to clipboard.")


def main() -> None:
	root = tk.Tk()
	app = DotStrokeApp(root)
	del app
	root.mainloop()


if __name__ == "__main__":
	main()
