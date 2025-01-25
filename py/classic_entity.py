from entities import Entity
import classic_component
import classic_handler
import dataclasses

class Player(Entity):
    keys = ["w","a","s","d","e","q"]
    def __post_init__(self):
        super().__post_init__()

        self.addComponent("transform2D",classic_component.Transform2D())
