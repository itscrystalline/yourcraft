# Import needs
from entities import Component
import dataclasses
from typing import Union, Any


@dataclasses.dataclass
class Position2D(Component):
    x: float = 0
    y: float = 0

    def __add__(self, other: Union["Position2D", tuple, list]):
        if isinstance(other, Position2D):
            return Position2D(x=self.x + other.x, y=self.y + other.y)
        else:
            return Position2D(x=self.x + other[0], y=self.y + other[1])

    def __sub__(self, other: Union["Position2D", tuple, list]):
        if isinstance(other, Position2D):
            return Position2D(x=self.x - other.x, y=self.y - other.y)
        else:
            return Position2D(x=self.x - other[0], y=self.y - other[1])

    def __mul__(self, other: float | int | complex):
        return Position2D(x=self.x * other, y=self.y * other)

    def __str__(self):
        return f"Position2D x={self.x}, y={self.y}"


@dataclasses.dataclass
class Velocity2D(Component):
    vx: float = 0
    vy: float = 0

    def __add__(self, other: Union["Velocity2D", tuple, list]):
        if isinstance(other, Velocity2D):
            return Velocity2D(vx=self.vx + other.vx, vy=self.vy + other.vy)
        else:
            return Velocity2D(vx=self.vx + other[0], vy=self.vy + other[1])

    def __sub__(self, other: Union["Velocity2D", tuple, list]):
        if isinstance(other, Velocity2D):
            return Velocity2D(vx=self.vx - other.vx, vy=self.vy - other.vy)
        else:
            return Velocity2D(vx=self.vx - other[0], vy=self.vy - other[1])

    def __mul__(self, other: float | int | complex):
        return Velocity2D(vx=self.vx * other, vy=self.vy * other)

    def __str__(self):
        return f"Velocity2D vx={self.vx}, vy={self.vy}"


@dataclasses.dataclass
class Acceleration2D(Component):
    ax: float = 0
    ay: float = 0

    def __add__(self, other: Union["Acceleration2D", tuple, list]):
        if isinstance(other, Acceleration2D):
            return Acceleration2D(ax=self.ax + other.ax, ay=self.ay + other.ay)
        else:
            return Acceleration2D(ax=self.ax + other[0], ay=self.ay + other[1])

    def __sub__(self, other: Union["Acceleration2D", tuple, list]):
        if isinstance(other, Acceleration2D):
            return Acceleration2D(ax=self.ax - other.ax, ay=self.ay - other.ay)
        else:
            return Acceleration2D(ax=self.ax - other[0], ay=self.ay - other[1])

    def __mul__(self, other: float | int | complex):
        return Acceleration2D(ax=self.ax * other, ay=self.ay * other)

    def __str__(self):
        return f"Acceleration2D ax={self.ax}, ay={self.ay}"


@dataclasses.dataclass
class Rotation2D(Component):
    _x: float = 0

    def __init__(self, x: float | int = 0) -> None:
        self._x = x
        super().__init__()

    @property
    def x(self) -> float:
        return self._x

    @x.setter
    def x(self, value: float) -> None:
        self._x = value % 360
        if self._x < 0:
            self._x += 360

    def __add__(self, other: Union["Rotation2D", float, int]):
        if isinstance(other, Rotation2D):
            return Rotation2D(x=self._x + other._x)
        else:
            return Rotation2D(x=self._x + other)

    def __sub__(self, other: Union["Rotation2D", float, int]):
        if isinstance(other, Rotation2D):
            return Rotation2D(x=self._x - other._x)
        else:
            return Rotation2D(x=self._x - other)

    def __mul__(self, other: float | int):
        return Rotation2D(x=self._x * other)


@dataclasses.dataclass
class Transform2D(Component):
    position: Position2D = dataclasses.field(default_factory=Position2D)
    rotation: Rotation2D = dataclasses.field(default_factory=Rotation2D)


@dataclasses.dataclass
class Health(Component):
    current: int = 100
    maximum: int = 100


@dataclasses.dataclass
class Cooldown(Component):
    current: float = 0
    maximum: float = 1


@dataclasses.dataclass
class ImageSprite(Component):
    image_path: str = None
    layer: int = 0


@dataclasses.dataclass
class Inventory(Component):
    items: dict[str, int] = dataclasses.field(default_factory=dict)
