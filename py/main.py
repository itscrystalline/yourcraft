import math
import sys
import pygame
import classic_component
import classic_entity
import network
import time
import threading
import os

# Initialize Pygame
pygame.init()

# Set up the display
screen_size = pygame.display.get_desktop_sizes()[0]
screen_width = screen_size[0]
screen_height = screen_size[1]
screen = pygame.display.set_mode((screen_width, screen_height),
                                 pygame.SRCALPHA | pygame.HWSURFACE | pygame.DOUBLEBUF | pygame.RESIZABLE, vsync=1)
pygame.display.set_caption("Pygame Initialization Example")

# Set pixel scaling
pixel_scaling = 20

# Clock
clock = pygame.time.Clock()

# Font
font = pygame.font.SysFont("Arial", 20)

# Set up colors
WHITE = (255, 255, 255)
BLUE = (0, 0, 255)

# Entities
currentPlayer = classic_entity.Player()
# K_RETURN is [Enter]
currentPlayer.keys = [pygame.K_a, pygame.K_d, pygame.K_e, pygame.K_q, pygame.K_SPACE, pygame.K_RETURN]
position2D = currentPlayer.getComponent("transform2D").getVariable("position")
speed = 10 * pixel_scaling

# Other players
otherPlayers = {}

# Messages' "queue"
messages = []
MAX_MESSAGES = 50
is_chatting = False
chat_key_pressing = False

# World
World = {}
WorldPosition = classic_component.Position2D()
WorldDelta = classic_component.Velocity2D()

# MousePos
MousePos = pygame.mouse.get_pos()


# Block Types
def load_resource(name):
    file = f"{os.path.dirname(os.path.realpath(__file__))}/resources/{name}"
    pic = pygame.image.load(file)
    pic = pygame.transform.scale_by(pic, 2).convert_alpha()
    return pic


BlockType = list(
    map(load_resource,
        ["grassblock.png", "stoneblock.png", "woodblock.png", "leaves.png", "waterblock.png", "background.png"]))

# Set connection
cliNet = network.ServerConnection("127.0.0.1")
cliNet.send(network.ClientHello("test"))

# Synchronize network initialization
INIT_DATA = cliNet.recv()['data']
print(INIT_DATA)

# Initialize Data
currentPlayer.player_id = INIT_DATA['player_id']
position2D.x = INIT_DATA['spawn_x'] * pixel_scaling
position2D.y = INIT_DATA['spawn_y'] * pixel_scaling
WorldPosition.x = -INIT_DATA['spawn_x'] * pixel_scaling
WorldPosition.y = -INIT_DATA['spawn_y'] * pixel_scaling
WasJump = False
prev_direction = 0

# Network thread with proper handling of shared resources
network_lock = threading.Lock()

ReadyToUpdate = {}

running = True


def NetworkThread():
    global World, position2D, netThread, running

    while True:
        time.sleep(0.016)  # Sleep for 16ms (for approx. 60FPS)
        receiving = cliNet.recv()
        print(receiving)

        # Synchronize access to the shared resource
        with network_lock:
            if receiving['t'] == network.KICK:
                print("kicked because", receiving['data']['msg'])
                running = False
                return
            elif receiving['t'] == network.HEARTBEAT_SERVER:
                cliNet.send(network.ClientHeartbeat())
            elif receiving['t'] == network.CHUNK_UPDATE:
                updated_chunk = receiving['data']['chunk']
                # Update the world data with the new chunk
                chunk_coord = (updated_chunk['chunk_x'], updated_chunk['chunk_y'])
                World[chunk_coord] = {}
                for i in range(0, updated_chunk['blocks'].__len__()):
                    World[chunk_coord][(i % 16, i // 16)] = updated_chunk['blocks'][
                        updated_chunk['blocks'].__len__() - 1 - i]
            elif receiving['t'] == network.PLAYER_UPDATE_POS:
                receivedPlayerID = receiving['data']['player_id']
                if receivedPlayerID == currentPlayer.player_id:
                    if network.PLAYER_UPDATE_POS not in ReadyToUpdate:
                        ReadyToUpdate[network.PLAYER_UPDATE_POS] = {}
                    ReadyToUpdate[network.PLAYER_UPDATE_POS][receivedPlayerID] = receiving['data']
                elif otherPlayers.get(receivedPlayerID) is not None:
                    otherPlayers[receivedPlayerID] = receiving['data']
                    del otherPlayers[receivedPlayerID]['player_id']
            elif receiving['t'] == network.PLAYER_ENTER_LOAD:
                otherPlayers[receiving['data']['player_id']] = receiving['data']
                del otherPlayers[receiving['data']['player_id']]['player_id']
            elif receiving['t'] == network.PLAYER_LEAVE_LOAD:
                try:
                    otherPlayers[receiving['data']['player_id']].clear()
                    del otherPlayers[receiving['data']['player_id']]
                except KeyError:
                    pass
            elif receiving['t'] == network.UPDATE_BLOCK:
                if (UpdateChunk := World.get((int(receiving['data']['x'] // 16),
                                              int(receiving['data']['y'] // 16)))) is not None:
                    print((15 - int(receiving['data']['x'] % 16 // pixel_scaling),
                           15 - int(receiving['data']['y'] % 16 // pixel_scaling)))
                    UpdateChunk[(15 - int(receiving['data']['x'] % 16),
                                 15 - int(receiving['data']['y'] % 16))] = \
                        receiving['data']['block']


# Draw world
def draw_world(chunkCoord):
    dChunkX = math.ceil(screen_width / 32 / pixel_scaling)
    dChunkY = math.ceil(screen_height / 32 / pixel_scaling)

    # Unload Chunk
    checkUnloadChunks = list(World.keys())
    for checkUnloadChunk in checkUnloadChunks:
        if not (chunkCoord[0] - dChunkX <= checkUnloadChunk[0] <= chunkCoord[0] + dChunkX + 1):
            cliNet.send(network.ClientUnloadChunk(checkUnloadChunk[0], checkUnloadChunk[1]))
            World[checkUnloadChunk].clear()
            del World[checkUnloadChunk]
            continue
        if not (chunkCoord[1] - dChunkY <= checkUnloadChunk[1] <= chunkCoord[1] + dChunkY + 1):
            cliNet.send(network.ClientUnloadChunk(checkUnloadChunk[0], checkUnloadChunk[1]))
            World[checkUnloadChunk].clear()
            del World[checkUnloadChunk]
            continue

    # Draw Chunk
    for loadChunkX in range(chunkCoord[0] - dChunkX, chunkCoord[0] + dChunkX + 1):
        for loadChunkY in range(chunkCoord[1] - dChunkY, chunkCoord[1] + dChunkY + 1):
            loadChunk = (loadChunkX, loadChunkY)
            if loadChunk in World:
                for blockPos, blockType in World[loadChunk].items():
                    blockScreenPos = (
                        loadChunk[0] * 16 * pixel_scaling - blockPos[
                            0] * pixel_scaling + WorldPosition.x + 14.5 * pixel_scaling + screen_width / 2,
                        -loadChunk[1] * 16 * pixel_scaling + blockPos[
                            1] * pixel_scaling - WorldPosition.y - 15 * pixel_scaling + screen_height / 2
                    )
                    if blockType > 0:
                        screen.blit(BlockType[blockType - 1], (blockScreenPos[0], blockScreenPos[1]))

            else:
                if (loadChunkX < 0) or (loadChunkY < 0):
                    continue
                World[loadChunk] = {}
                cliNet.send(network.ClientRequestChunk(loadChunk[0], loadChunk[1]))


# Draw other players
def draw_other_players():
    for eachPlayer in otherPlayers.values():
        pygame.draw.rect(screen, WHITE, (
            eachPlayer['pos_x'] * pixel_scaling - position2D.x + screen_width / 2 - pixel_scaling / 2,
            position2D.y - eachPlayer['pos_y'] * pixel_scaling + screen_height / 2 - pixel_scaling, pixel_scaling,
            2 * pixel_scaling))


# Sync Server
def sync_data():
    global ReadyToUpdate
    for protocolType, protocolValue in ReadyToUpdate.items():
        match protocolType:
            case network.PLAYER_UPDATE_POS:
                for updatePlayerID, rawPosition in protocolValue.items():
                    if currentPlayer.player_id == updatePlayerID and rawPosition.__len__() != 0:
                        newX = rawPosition['pos_x'] * pixel_scaling
                        position2D.x = newX
                        WorldPosition.x = -position2D.x
                        newY = rawPosition['pos_y'] * pixel_scaling
                        position2D.y = newY
                        WorldPosition.y = -position2D.y

                protocolValue.clear()


bg = pygame.image.load("resources/background.png").convert_alpha()


# Get block
def get_block(x, y) -> int:
    return World[(int(x // (16 * pixel_scaling)), int(y // (16 * pixel_scaling)))] \
        [(15 - int(x % (16 * pixel_scaling) // pixel_scaling), 15 - int(y % (16 * pixel_scaling) // pixel_scaling))]


# Define placement range
def place_in_range(x, y, d, b=2) -> bool:
    if (d[0] ** 2 + d[1] ** 2) <= 64 or (d[0] ** 2 + (d[1] - 1) ** 2) <= 64:
        cliNet.send(network.ClientPlaceBlock(b, x, y))
        return True
    return False


# Game loop
def main():
    global running, screen_size, screen_width, screen_height, WasJump, prev_direction, MousePos, is_chatting, chat_key_pressing
    while running:
        dt = clock.tick(50) / 1000  # Calculate time per frame
        MousePos = pygame.mouse.get_pos()
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                cliNet.send(network.ClientGoodbye())
                pygame.quit()
                running = False
            elif event.type == pygame.VIDEORESIZE:
                screen_size = screen.get_size()
                screen_width = screen_size[0]
                screen_height = screen_size[1]

        # Update from server :)
        sync_data()

        # Update movement / controls
        movement_update = False
        WorldDelta.setVariable(vx=0, vy=0)
        keys = pygame.key.get_pressed()

        chunkCoord = (int(position2D.x // (16 * pixel_scaling)), int(position2D.y // (16 * pixel_scaling)))

        need_update_pos = False
        speed_update = 0
        if not is_chatting:
            if keys[currentPlayer.keys[0]]:  # Move left
                position2D.x -= speed * dt
                WorldDelta.vx -= speed * dt
                movement_update = True
                if prev_direction != -1:
                    need_update_pos = True
                    speed_update = -speed * dt
                    prev_direction = -1
            elif keys[currentPlayer.keys[1]]:  # Move right
                position2D.x += speed * dt
                WorldDelta.vx += speed * dt
                movement_update = True
                if prev_direction != 1:
                    need_update_pos = True
                    speed_update = speed * dt
                    prev_direction = 1
            else:
                if prev_direction != 0:
                    print("stopped")
                    movement_update = True
                    need_update_pos = True
                    speed_update = 0
                    prev_direction = 0

            if keys[currentPlayer.keys[2]]:  # Place block
                NormalX = int((position2D.x - screen_width / 2 + MousePos[0] + pixel_scaling / 2) // pixel_scaling)
                NormalY = int((position2D.y + screen_height / 2 - MousePos[1] + pixel_scaling) // pixel_scaling)
                print(NormalX, NormalY)
                if NormalX >= 0 and NormalY >= 0:
                    dScreenMouse = ((MousePos[0] - screen_width / 2) / pixel_scaling,
                                    (MousePos[1] - screen_height / 2) / pixel_scaling)
                    place_in_range(NormalX, NormalY, dScreenMouse, 2)

            if keys[currentPlayer.keys[3]]:  # Remove block
                NormalX = int((position2D.x - screen_width / 2 + MousePos[0] + pixel_scaling / 2) // pixel_scaling)
                NormalY = int((position2D.y + screen_height / 2 - MousePos[1] + pixel_scaling) // pixel_scaling)
                print(NormalX, NormalY)
                if NormalX >= 0 and NormalY >= 0:
                    dScreenMouse = ((MousePos[0] - screen_width / 2) / pixel_scaling,
                                    (MousePos[1] - screen_height / 2) / pixel_scaling)
                    place_in_range(NormalX, NormalY, dScreenMouse, 0)

            if keys[currentPlayer.keys[4]]:  # Jump
                if not WasJump:
                    cliNet.send(network.ClientPlayerJump())
                    WasJump = True
            else:
                WasJump = False

        # Enable chatting
        if keys[currentPlayer.keys[5]] and is_chatting and not chat_key_pressing:
            chat_key_pressing = True
            is_chatting = False
            cliNet.send(network.ClientSendMessage("FUCK YOU"))
        elif keys[currentPlayer.keys[5]] and not is_chatting and not chat_key_pressing:
            is_chatting = True
            chat_key_pressing = True
        elif not keys[currentPlayer.keys[5]] and chat_key_pressing:
            chat_key_pressing = False


        # # Debug chunk
        # if keys[pygame.K_EQUALS]:
        #     cliNet.send(network.ClientPlayerXVelocity(0))
        # if keys[pygame.K_w]:  # Move up
        #     position2D.y += speed * dt
        #     WorldDelta.vy += speed * dt
        #     movement_update = True
        # if keys[pygame.K_s]:  # Move down
        #     position2D.y -= speed * dt
        #     WorldDelta.vy -= speed * dt
        #     movement_update = True

        # Move world
        if movement_update:
            WorldPosition.x -= WorldDelta.vx
            WorldPosition.y -= WorldDelta.vy
            if need_update_pos:
                print("sending velocity")
                cliNet.send(network.ClientPlayerXVelocity(speed_update / pixel_scaling))

        # Draw background
        screen.blit(bg, (0, 0))

        # Draw world (visible chunks)
        draw_world(chunkCoord)

        # Draw other players
        draw_other_players()

        # Draw player
        pygame.draw.rect(screen, WHITE, (
            screen_width / 2 - pixel_scaling / 2, screen_height / 2 - pixel_scaling, pixel_scaling, 2 * pixel_scaling))

        # Debug FPS and Position
        screen.blit(font.render(f"{clock.get_fps():.2f} FPS", 1, WHITE), (0, 0))
        screen.blit(font.render(f"{position2D}", 1, WHITE), (400, 0))

        # Update the display
        pygame.display.update()


# Quit Pygame
if __name__ == '__main__':
    netThread = threading.Thread(target=NetworkThread, daemon=True)
    netThread.start()
    main()

pygame.quit()
sys.exit()
