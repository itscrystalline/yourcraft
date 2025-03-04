import math
import os
import sys
import threading
import time

import pygame
import pygame.gfxdraw

import classic_component
import classic_entity
import network

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
pixel_scaling = 25

# Clock
clock = pygame.time.Clock()

# Font
font = pygame.font.SysFont("Arial", 20)
message_font = pygame.font.SysFont("Arial", 60)

scene_state = 0

player_name = "player"

# Set up colors
WHITE = (255, 255, 255)
BLUE = (0, 0, 255)

# Entities
currentPlayer = classic_entity.Player()
# K_RETURN is [Enter]
currentPlayer.keys = [pygame.K_a, pygame.K_d, pygame.K_e, pygame.K_q, pygame.K_SPACE, pygame.K_RETURN, pygame.K_1,
                      pygame.K_2, pygame.K_3, pygame.K_4, pygame.K_5, pygame.K_6, pygame.K_7, pygame.K_8, pygame.K_9]
position2D = currentPlayer.getComponent("transform2D").getVariable("position")
speed = 5 * pixel_scaling
playerInventory = currentPlayer.getComponent("inventory").getVariable("items")
playerInventory[0] = {'item': -1, 'count': 0}
playerInventory[1] = {'item': -1, 'count': 0}
playerInventory[2] = {'item': -1, 'count': 0}
playerInventory[3] = {'item': -1, 'count': 0}
playerInventory[4] = {'item': -1, 'count': 0}
playerInventory[5] = {'item': -1, 'count': 0}
playerInventory[6] = {'item': -1, 'count': 0}
playerInventory[7] = {'item': -1, 'count': 0}
playerInventory[8] = {'item': -1, 'count': 0}
playerSelectedSlot = currentPlayer.getComponent("selectedSlot")
lookLeft = True
# Other players
otherPlayers = {}

# Messages' "queue"
messages = []
client_message = ""
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
def load(name):
    file = f"{os.path.dirname(os.path.realpath(__file__))}/resources/{name}"
    return pygame.image.load(file)


def load_resource(name):
    pic = load(name)
    pic = pygame.transform.scale_by(pic, pixel_scaling / 10).convert_alpha()
    return pic


BlockType = list(
    map(load_resource,
        ["grassblock.png", "stoneblock.png", "woodblock.png", "leaves.png", "waterblock.png", "ore.png"]))
bg = pygame.transform.scale_by(load("background2.png"), pixel_scaling / 15).convert_alpha()
items = list(map(load_resource, ["sword.png", "axe.png", "pickaxe.png"]))


# Run only change resolution
def reload_resource():
    global BlockType, bg, items
    BlockType = list(
        map(load_resource,
            ["grassblock.png", "stoneblock.png", "woodblock.png", "leaves.png", "waterblock.png", "ore.png"]))
    bg = pygame.transform.scale_by(load("background2.png"), pixel_scaling / 15).convert_alpha()
    items = list(map(load_resource, ["sword.png", "axe.png", "pickaxe.png"]))


Non_Solid = [0, 5]
itemsByID = [BlockType[0], BlockType[1], BlockType[2], BlockType[3], bg, BlockType[4], items[2], items[1], items[0],
             BlockType[5]]

# Set connection
cliNet = network.ServerConnection("127.0.0.1")
cliNet.send(network.ClientHello(player_name))

# Synchronize network initialization
INIT_DATA = cliNet.recv()['data']

# Initialize Data
currentPlayer.player_id = INIT_DATA['player_id']
position2D.x = INIT_DATA['spawn_x'] * pixel_scaling
position2D.y = INIT_DATA['spawn_y'] * pixel_scaling
WorldPosition.x = -INIT_DATA['spawn_x'] * pixel_scaling
WorldPosition.y = -INIT_DATA['spawn_y'] * pixel_scaling
Worldwidth = INIT_DATA['world_width']
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
        # print(receiving)

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
                    UpdateChunk[(15 - int(receiving['data']['x'] % 16),
                                 15 - int(receiving['data']['y'] % 16))] = \
                        receiving['data']['block']
            elif receiving['t'] == network.BATCH_UPDATE_BLOCK:
                for x, y in receiving['data']['batch']:
                    if (UpdateChunk := World.get((int(x // 16),
                                                  int(y // 16)))) is not None:
                        UpdateChunk[(15 - int(x % 16),
                                     15 - int(y % 16))] = receiving['data']['block']
            elif receiving['t'] == network.UPDATE_INVENTORY:
                e = -1
                for item_in_slot in receiving['data']['inv']:
                    e += 1
                    if item_in_slot is None:
                        playerInventory[e] = {'item': -1, 'count': 0}
                        continue
                    playerInventory[e] = item_in_slot
            elif receiving['t'] == network.SERVER_MESSAGE:
                print(receiving['data'])
                if messages.__len__() == MAX_MESSAGES:
                    messages.pop(0)

                messages.append("[" + receiving['data']['player_name'] + "] " + receiving['data']['msg'])


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


# Get block
def get_block(x, y) -> int:
    try:
        if x < 0 or y < 0:
            return -1
        return World[(int(x // (16 * pixel_scaling)), int(y // (16 * pixel_scaling)))] \
            [(15 - int(x % (16 * pixel_scaling) // pixel_scaling), 15 - int(y % (16 * pixel_scaling) // pixel_scaling))]
    except:
        return -1


# Define placement range
def place_in_range(x, y, d) -> bool:
    if (d[0] ** 2 + d[1] ** 2) <= 64 or (d[0] ** 2 + (d[1] - 1) ** 2) <= 64:
        # if (UpdateChunk := World.get((int(x // 16), int(y // 16)))) is not None:
        # UpdateChunk[(15 - int(x % 16), 15 - int(y % 16))] = -2
        cliNet.send(network.ClientPlaceBlock(x, y))
        return True
    return False


# Define break range
def break_in_range(x, y, d) -> bool:
    if (d[0] ** 2 + d[1] ** 2) <= 64 or (d[0] ** 2 + (d[1] - 1) ** 2) <= 64:
        cliNet.send(network.ClientBreakBlock(x, y))
        return True
    return False


# Game loop
def main():
    global running, screen_size, screen_width, screen_height, WasJump, prev_direction, MousePos, is_chatting \
        , chat_key_pressing, client_message, playerSelectedSlot, pixel_scaling, lookLeft, scene_state
    while running:
        if scene_state == 0:
            screen.fill((0,0,0))
            continue
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
                reload_resource()
            elif event.type == pygame.KEYDOWN:
                if is_chatting:
                    if event.key == pygame.K_BACKSPACE and client_message.__len__() > 0:
                        client_message = client_message[:-1]
                    elif event.key == pygame.K_RETURN:
                        continue
                    else:
                        client_message += event.unicode
                elif event.key in currentPlayer.keys[6:15]:
                    cliNet.send(network.ClientChangeSlot(event.key - 49))
                    playerSelectedSlot.slot = event.key - 49
            elif event.type == pygame.MOUSEBUTTONDOWN:
                mouse = pygame.mouse.get_pressed(3)
                print(mouse)
                mouse_left, _, mouse_right = mouse
                if mouse_right:
                    NormalX = int((position2D.x - screen_width / 2 + MousePos[0] + pixel_scaling / 2) // pixel_scaling)
                    NormalY = int((position2D.y + screen_height / 2 - MousePos[1] + pixel_scaling) // pixel_scaling)
                    # print(NormalX, NormalY)
                    if NormalX >= 0 and NormalY >= 0:
                        dScreenMouse = ((MousePos[0] - screen_width / 2) / pixel_scaling,
                                        (MousePos[1] - screen_height / 2) / pixel_scaling)
                        place_in_range(NormalX, NormalY, dScreenMouse)
                elif mouse_left:
                    NormalX = int((position2D.x - screen_width / 2 + MousePos[0] + pixel_scaling / 2) // pixel_scaling)
                    NormalY = int((position2D.y + screen_height / 2 - MousePos[1] + pixel_scaling) // pixel_scaling)
                    # print(NormalX, NormalY)
                    if NormalX >= 0 and NormalY >= 0:
                        dScreenMouse = ((MousePos[0] - screen_width / 2) / pixel_scaling,
                                        (MousePos[1] - screen_height / 2) / pixel_scaling)
                        break_in_range(NormalX, NormalY, dScreenMouse)

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
            if keys[currentPlayer.keys[0]] and (get_block(position2D.x - 1, position2D.y) in Non_Solid) and (
                    get_block(position2D.x - 1, position2D.y + pixel_scaling) in Non_Solid):  # Move left
                position2D.x -= speed * dt
                WorldDelta.vx -= speed * dt
                movement_update = True
                if prev_direction != -1:
                    need_update_pos = True
                    speed_update = -speed * dt
                    prev_direction = -1
                lookLeft = True
            elif keys[currentPlayer.keys[1]] and (
                    get_block(position2D.x + pixel_scaling, position2D.y) in Non_Solid) and (
                    get_block(position2D.x + pixel_scaling, position2D.y + 1) in Non_Solid):  # Move right
                position2D.x += speed * dt
                WorldDelta.vx += speed * dt
                movement_update = True
                if prev_direction != 1:
                    need_update_pos = True
                    speed_update = speed * dt
                    prev_direction = 1
                lookLeft = False
            else:
                if prev_direction != 0:
                    movement_update = True
                    need_update_pos = True
                    speed_update = 0
                    prev_direction = 0

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
            if client_message != "":
                cliNet.send(network.ClientSendMessage(client_message))
                client_message = ""
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
        Worldwidth_percent = (position2D.x / pixel_scaling) / Worldwidth
        movable_width = 7680 - screen_width
        X_value = Worldwidth_percent * movable_width
        screen.blit(bg, (-X_value, 0))

        # Draw world (visible chunks)
        draw_world(chunkCoord)

        # Draw other players
        draw_other_players()

        # Draw player
        pygame.draw.rect(screen, WHITE, (
            screen_width / 2 - pixel_scaling / 2, screen_height / 2 - pixel_scaling, pixel_scaling, 2 * pixel_scaling))

        # Draw player's name
        name = font.render(player_name, 1, WHITE)
        name_rect = name.get_rect()

        screen.blit(name, (
            screen_width / 2 - name_rect.center[0], screen_height / 2 - 2 * pixel_scaling - name_rect.center[1]))

        if is_chatting:
            # Draw client chat
            pygame.gfxdraw.box(screen, (0, screen_height * 5 / 6 - screen_height / 10, screen_width, screen_height / 15),
                               (0, 0, 0, 64))
            screen.blit(message_font.render(client_message, 1, WHITE), (0, screen_height * 5 / 6 - screen_height / 10))
            if (msg_len := messages.__len__()) > 0:
                pygame.gfxdraw.box(screen,
                                   (0, screen_height * 5 / 6 - screen_height / 10 * (msg_len + 1), screen_width,
                                    screen_height / 10 * msg_len),
                                   (0, 0, 0, 64))
            e = 1
            for draw_message in messages[::-1]:
                e += 1
                screen.blit(message_font.render(draw_message, 1, WHITE),
                            (0, screen_height * 5 / 6 - screen_height / 10 * e))

        # Draw hotbar
        pygame.gfxdraw.box(screen, (screen_width / 3, screen_height * 5 / 6, screen_width / 3, screen_height / 15),
                           (0, 0, 0, 64))

        dSlot = screen_width / 27

        pygame.draw.rect(screen, WHITE, (
            screen_width / 3, screen_height * 5 / 6, screen_width / 3 + pixel_scaling // 4,
            screen_height / 15 + pixel_scaling // 4),
                         pixel_scaling // 4)

        # Draw items
        for slot_index in range(0, 9):
            if slot_index == playerSelectedSlot.slot:
                pygame.draw.rect(screen, WHITE, (
                    screen_width / 3 + dSlot * slot_index - pixel_scaling // 8,
                    screen_height * 5 / 6 - pixel_scaling // 8, dSlot + pixel_scaling // 2,
                    screen_height / 15 + pixel_scaling // 2),
                                 int(pixel_scaling // 4 * 1.5))
                if playerInventory[slot_index]['item'] == -1:
                    continue
                screen.blit(itemsByID[playerInventory[slot_index]['item']], (
                screen_width / 2 + (-pixel_scaling * 1.2 if lookLeft else pixel_scaling * 0.2),
                screen_height / 2 - pixel_scaling * 0.4))
            if playerInventory[slot_index]['item'] == -1:
                continue
            mul = slot_index * dSlot + dSlot / 3
            mul += screen_width / 3
            screen.blit(itemsByID[playerInventory[slot_index]['item']],
                        (mul, screen_height * 5 / 6 + screen_height / 15 / 3))
            item_count_font = font.render(playerInventory[slot_index]['count'].__str__(), 1, WHITE)
            item_count_font_rect = item_count_font.get_rect(
                midright=(mul + pixel_scaling, screen_height * 5 / 6 + screen_height / 15 / 1.414))
            screen.blit(item_count_font, item_count_font_rect)
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
