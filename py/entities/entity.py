# Entity Class
# All objects in game must be entity

import dataclasses
import uuid
import copy
from typing import TypeVar
from component import Component

T = TypeVar('T', bound=Component)

@dataclasses.dataclass
class Entity:
    __entity_id: str = dataclasses.field(init=False)
    __components: dict[str, Component] = dataclasses.field(default_factory=dict)

    def __post_init__(self):
        self.__entity_id = uuid.uuid4().hex

    # You should discard component you added after call this function
    def addComponent(self, key: str, component: Component) -> None:
        self.__components[key] = component

    def removeComponent(self, key: str) -> None:
        del self.__components[key]

    def getComponent(self, key: str) -> Component:
        return self.__components[key]

    def hasComponent(self, key: str) -> bool:
        return key in self.__components

    def tryAddComponent(self, key: str, component: Component) -> None:
        if key not in self.__components:
            self.addComponent(key, component)

    def tryGetComponent(self, key: str) -> Component:
        return self.__components[key]

    def setComponent(self, key: str, component: Component) -> None:
        self.getComponent(key).setVariable(component.getVariable())

    # Don't use this if possible
    def followComponent(self, key: str, component: Component) -> None:
        self.addComponent(key, component)

    # Don't use this if possible
    def unfollowComponent(self, key: str) -> None:
        self.addComponent(key, copy.deepcopy(self.getComponent(key)))

    def __eq__(self, other: "Entity") -> bool:
        return self.__entity_id == other.__entity_id

    def __repr__(self) -> str:
        return f"Entity(id={self.__entity_id}, components={self.__components})"
