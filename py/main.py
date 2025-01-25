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
speed = 0.1

# World
WorldSurface = pygame.Surface((screen_width, screen_height))
WorldSurface.fill(WHITE)
World = {}
# Format as (x, y) : int of block type

# Block Types (In dev)
BlockType = [(0,0,0),(255,0,0),(0,255,0),(0,0,255),(255,255,0),(255,0,255),(0,255,255),(255,255,255)]

# Game loop
def main():
    running = True
    while running:
        for event in pygame.event.get():
            match event.type:
                case pygame.QUIT:
                    pygame.quit()
                    running = False
                case _:
                    continue

        # Update movement / controls
        keys = pygame.key.get_pressed()
        if keys[currentPlayer.keys[0]]:
            position2D.y -= 1 * speed
        if keys[currentPlayer.keys[1]]:
            position2D.x -= 1 * speed
        if keys[currentPlayer.keys[2]]:
            position2D.y += 1 * speed
        if keys[currentPlayer.keys[3]]:
            position2D.x += 1 * speed
        if keys[currentPlayer.keys[4]]:
            World[(position2D.x//10*10, position2D.y//10*10+10)] = 2
            pygame.draw.rect(WorldSurface, BlockType[2], (position2D.x//10*10, position2D.y//10*10+10, 10, 10))
        if keys[currentPlayer.keys[5]]:
            pass

        # # Reset/Clear screen
        # screen.fill(WHITE)
        # # No longer needed, as we draw world instead

        # Draw world
        screen.blit(WorldSurface, (0, 0))

        # Draw player
        pygame.draw.rect(screen, BLUE, (position2D.x, position2D.y, 10, 20))

        # Debug FPS
        clock.tick()
        screen.blit(font.render(f"{clock.get_fps().__format__('.2f')} FPS", 1, (0,0,0)),(0,0))

        # Update the display
        pygame.display.flip()

# Quit Pygame
if __name__ == '__main__':
    main()
pygame.quit()
sys.exit()
