from entities import Entity
import classic_component

class Player(Entity):
    player_id = None
    def __post_init__(self):
        super().__post_init__()

        self.addComponent("transform2D",classic_component.Transform2D())
        self.addComponent("velocity",classic_component.Velocity2D())
        self.addComponent("acceleration",classic_component.Acceleration2D())
        self.addComponent("inventory",classic_component.Inventory())
        self.addComponent("selectedSlot",classic_component.SelectedSlot())
        self.addComponent("health",classic_component.Health())
