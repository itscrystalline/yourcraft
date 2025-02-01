import pygame
import classic_component
import classic_handler
import classic_entity
import entities
import sys
import threading
import multiprocessing
import math

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
World = {}
WorldPosition = classic_component.Position2D()
WorldDelta = classic_component.Velocity2D()
# Format as { (x, y) of chunk : chunk data->{ coord in chunk (x, y) : block } }


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

        chunkCoord = (int(position2D.x//160), int(position2D.y//160))
        chunkPos = (int(position2D.x%160//10*10), int(position2D.y%160//10*10))

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
            if chunkCoord not in World:
                World[chunkCoord] = {}
            World[chunkCoord][chunkPos] = 2
        if keys[currentPlayer.keys[5]]:
            if World.get(chunkCoord).get(chunkPos) is not None:
                del World[chunkCoord][chunkPos]
                if World[chunkCoord].__len__() == 0:
                    del World[chunkCoord]

        # Reset screen
        screen.fill((130,200,229))
        # Bring back because we need sky lol

        # Move world
        if movement_update:
            WorldPosition.x -= WorldDelta.vx
            WorldPosition.y -= WorldDelta.vy

        # Draw world
        dChunkX = math.ceil(screen_width/320)+1
        dChunkY = math.ceil(screen_height/320)+1

        # Choose visible chunks
        for loadChunkX in range(chunkCoord[0] - dChunkX, chunkCoord[0] + dChunkX):
            for loadChunkY in range(chunkCoord[1] - dChunkY, chunkCoord[1] + dChunkY):
                loadChunk = (loadChunkX, loadChunkY)
                if World.get(loadChunk):
                    # Draw blocks
                    for blockPos, blockType in World[loadChunk].items():
                        blockScreenPos = (loadChunk[0] * 160 + blockPos[0] + WorldPosition.x + screen_width / 2,
                                          loadChunk[1] * 160 + blockPos[1] + WorldPosition.y + 10 + screen_height / 2)
                        pygame.draw.rect(screen, BlockType[blockType], (blockScreenPos[0], blockScreenPos[1], 10, 10))

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
