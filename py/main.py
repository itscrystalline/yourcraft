import pygame
import classic_component
import classic_handler
import classic_entity
import entities
import sys
import threading
import multiprocessing

# Initialize Pygame
pygame.init()

# Set up the display
screen_width = 800
screen_height = 600
screen = pygame.display.set_mode((screen_width, screen_height), pygame.SRCALPHA | pygame.HWSURFACE | pygame.DOUBLEBUF)
pygame.display.set_caption("Pygame Initialization Example")

# Clock
clock = pygame.time.Clock()

# Font
font = pygame.font.SysFont("Arial", 20)

# Set up colors
WHITE = (255, 255, 255)
BLUE = (0, 0, 255)

# Entities
currentPlayer = classic_entity.Player()
currentPlayer.keys = [pygame.K_w,pygame.K_a,pygame.K_s,pygame.K_d,pygame.K_e,pygame.K_q]
position2D = currentPlayer.getComponent("transform2D").getVariable("position")
speed = 100

# World
WorldSurface = pygame.Surface((screen_width, screen_height), pygame.SRCALPHA)
World = {}
WorldRect = WorldSurface.get_rect()
WorldPosition = classic_component.Position2D()
WorldDelta = classic_component.Velocity2D()
# Format as (x, y) : int of block type


# Block Types (In dev)
BlockType = [(0,0,0),(255,0,0),(0,255,0),(0,0,255),(255,255,0),(255,0,255),(0,255,255),(255,255,255)]

# Game loop
def main():
    running = True
    while running:
        dt = clock.tick()/1000
        for event in pygame.event.get():
            match event.type:
                case pygame.QUIT:
                    pygame.quit()
                    running = False
                case _:
                    continue

        # Update movement / controls
        movement_update = False
        WorldDelta.setVariable(vx=0, vy=0)
        keys = pygame.key.get_pressed()

        posNow = (position2D.x//10*10+screen_width/2, position2D.y//10*10+10+screen_height/2)

        if keys[currentPlayer.keys[0]]:
            position2D.y -= speed * dt
            WorldDelta.vy -= speed * dt
            movement_update = True
        if keys[currentPlayer.keys[1]]:
            position2D.x -= speed * dt
            WorldDelta.vx -= speed * dt
            movement_update = True
        if keys[currentPlayer.keys[2]]:
            position2D.y += speed * dt
            WorldDelta.vy += speed * dt
            movement_update = True
        if keys[currentPlayer.keys[3]]:
            position2D.x += speed * dt
            WorldDelta.vx += speed * dt
            movement_update = True
        if keys[currentPlayer.keys[4]]:
            World[posNow] = 2
            pygame.draw.rect(WorldSurface, BlockType[2], (posNow[0],posNow[1], 10, 10))
        if keys[currentPlayer.keys[5]]:
            if World.get(posNow) is not None:
                del World[posNow]
                pygame.draw.rect(WorldSurface,(0,0,0,0), (posNow[0],posNow[1], 10, 10))

        # Reset screen
        screen.fill((130,200,229))
        # Bring back because we need sky lol

        # Move world
        if movement_update:
            WorldPosition.x -= WorldDelta.vx
            WorldPosition.y -= WorldDelta.vy
            WorldRect.x = WorldPosition.x
            WorldRect.y = WorldPosition.y


        # Draw world
        screen.blit(WorldSurface, WorldRect.topleft)

        # Draw player
        pygame.draw.rect(screen, BLUE, (screen_width/2-5, screen_height/2-10, 10, 20))

        # Debug FPS
        screen.blit(font.render(f"{clock.get_fps().__format__('.2f')} FPS", 1, (0,0,0)),(0,0))

        # Update the display
        pygame.display.flip()

# Quit Pygame
if __name__ == '__main__':
    main()
pygame.quit()
sys.exit()
