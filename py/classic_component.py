# Import needs
from entities import Component
import dataclasses


@dataclasses.dataclass
class Position2D(Component):
    x : float = 0
    y : float = 0


@dataclasses.dataclass
class Rotation2D(Component):
    x : float = 0


@dataclasses.dataclass
class Transform2D(Component):
    position : Position2D = dataclasses.field(default_factory=Position2D)
    rotation : Rotation2D = dataclasses.field(default_factory=Rotation2D)


@dataclasses.dataclass
class Health(Component):
    current : int = 100
    maximum : int = 100


@dataclasses.dataclass
class Cooldown(Component):
    current : float = 0
    maximum : float = 1


@dataclasses.dataclass
class ImageSprite(Component):
    image_path : str = None
    layer : int = 0


@dataclasses.dataclass
class Inventory(Component):
    items : dict[str, int] = dataclasses.field(default_factory=dict)
