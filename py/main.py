import math
import sys
import pygame
import classic_component
import classic_entity
import network
import time
import threading

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
currentPlayer.keys = [pygame.K_w, pygame.K_a, pygame.K_s, pygame.K_d, pygame.K_e, pygame.K_q]
position2D = currentPlayer.getComponent("transform2D").getVariable("position")
speed = 100

otherPlayers = []

# World
World = {}
WorldPosition = classic_component.Position2D()
WorldDelta = classic_component.Velocity2D()

# Block Types (In dev)
BlockType = [(0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (255, 0, 255), (0, 255, 255),
             (255, 255, 255)]

# Set connection

cliNet = network.ServerConnection("127.0.0.1")
cliNet.send(network.Hello("test"))

# Synchronize network initialization
INIT_DATA = cliNet.recv()['data']
print(INIT_DATA)

# Initialize Data
currentPlayer.player_id = INIT_DATA['player_id']
position2D.x = INIT_DATA['spawn_x'] * 10
position2D.y = INIT_DATA['spawn_y'] * 10
WorldPosition.x = INIT_DATA['spawn_x'] * 10
WorldPosition.y = INIT_DATA['spawn_y'] * 10

# Network thread with proper handling of shared resources
network_lock = threading.Lock()

def NetworkThread():
    global World, position2D

    while True:
        time.sleep(0.016)  # Sleep for 16ms (for approx. 60FPS)
        receiving = cliNet.recv()

        # Synchronize access to the shared resource
        with network_lock:
            print(receiving)
            if receiving['t'] == network.KICK:
                print("kicked because", receiving['data']['msg'])
                exit(0)
            elif receiving['t'] == network.HEARTBEAT_SERVER:
                print("heartbeat received")
                cliNet.send(network.Heartbeat())
            elif receiving['t'] == network.CHUNK_UPDATE:
                updated_chunk = receiving['data']['chunk']
                # Update the world data with the new chunk
                chunk_coord = (updated_chunk['chunk_x'], updated_chunk['chunk_y']-1)
                World[chunk_coord] = {}
                for i in range(0, updated_chunk['blocks'].__len__()):
                    World[chunk_coord][(i % 16 * 10, i // 16 * 10)] = updated_chunk['blocks'][updated_chunk['blocks'].__len__() - 1 - i]
            else:
                print(receiving)

# Game loop
def main():
    running = True
    while running:
        dt = clock.tick(50) / 1000  # Calculate time per frame
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                cliNet.send(network.Goodbye())
                pygame.quit()
                running = False

        # Update movement / controls
        movement_update = False
        WorldDelta.setVariable(vx=0, vy=0)
        keys = pygame.key.get_pressed()

        chunkCoord = (int(position2D.x // 160), int(position2D.y // 160))
        chunkPos = (int(position2D.x % 160 // 10 * 10), int(position2D.y % 160 // 10 * 10))

        if keys[currentPlayer.keys[0]]:  # Move up
            position2D.y += speed * dt
            WorldDelta.vy += speed * dt
            movement_update = True
        if keys[currentPlayer.keys[1]]:  # Move left
            position2D.x -= speed * dt
            WorldDelta.vx -= speed * dt
            movement_update = True
        if keys[currentPlayer.keys[2]]:  # Move down
            position2D.y -= speed * dt
            WorldDelta.vy -= speed * dt
            movement_update = True
        if keys[currentPlayer.keys[3]]:  # Move right
            position2D.x += speed * dt
            WorldDelta.vx += speed * dt
            movement_update = True
        if keys[currentPlayer.keys[4]]:  # Place block
            if chunkCoord not in World:
                World[chunkCoord] = {}
            World[chunkCoord][chunkPos] = 2
        if keys[currentPlayer.keys[5]]:  # Remove block
            if World.get(chunkCoord).get(chunkPos) is not None:
                del World[chunkCoord][chunkPos]
                if len(World[chunkCoord]) == 0:
                    del World[chunkCoord]

        # Reset screen
        screen.fill((130, 200, 229))

        # Move world
        if movement_update:
            WorldPosition.x -= WorldDelta.vx
            WorldPosition.y -= WorldDelta.vy

        # Draw world (visible chunks)
        dChunkX = math.ceil(screen_width / 320)
        dChunkY = math.ceil(screen_height / 320)

        for loadChunkX in range(chunkCoord[0] - dChunkX, chunkCoord[0] + dChunkX + 2):
            for loadChunkY in range(chunkCoord[1] - dChunkY, chunkCoord[1] + dChunkY + 3):
                loadChunk = (loadChunkX, loadChunkY)
                if loadChunk in World:
                    for blockPos, blockType in World[loadChunk].items():
                        blockScreenPos = (
                            loadChunk[0] * 160 - blockPos[0] + WorldPosition.x + 145 + screen_width / 2,
                            -loadChunk[1] * 160 + blockPos[1] - WorldPosition.y - 230 + screen_height / 2
                        )
                        pygame.draw.rect(screen, BlockType[blockType], (blockScreenPos[0], blockScreenPos[1], 10, 10))
                else:
                    if (loadChunkX < 0) or (loadChunkY < 0):
                        continue
                    World[loadChunk] = {}
                    print(loadChunk)
                    cliNet.send(network.ClientRequestChunk(loadChunk[0], loadChunk[1]))

        # Draw player
        pygame.draw.rect(screen, BLUE, (screen_width / 2 - 5, screen_height / 2 - 10, 10, 20))

        # Debug FPS and Position
        screen.blit(font.render(f"{clock.get_fps():.2f} FPS", 1, (0, 0, 0)), (0, 0))
        screen.blit(font.render(f"{position2D}", 1, (0, 0, 0)), (400, 0))

        # Update the display
        pygame.display.flip()

# Quit Pygame
if __name__ == '__main__':
    netThread = threading.Thread(target=NetworkThread, daemon=True)
    netThread.start()
    main()

pygame.quit()
sys.exit()
