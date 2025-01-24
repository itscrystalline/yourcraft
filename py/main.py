import pygame
import classic_component
import classic_handler
import classic_entity
import entities
import sys

# Initialize Pygame
pygame.init()

# Set up the display
screen_width = 800
screen_height = 600
screen = pygame.display.set_mode((screen_width, screen_height))
pygame.display.set_caption("Pygame Initialization Example")

# Set up colors
WHITE = (255, 255, 255)
BLUE = (0, 0, 255)

currentPlayer = classic_entity.Player()
currentPlayer.keys = [pygame.K_w,pygame.K_a,pygame.K_s,pygame.K_d]
print(currentPlayer.__dict__)
position2D = currentPlayer.getComponent("transfrom2D").getVariable("position")

# Game loop
running = True
while running:
    for event in pygame.event.get():
        match event.type:
            case pygame.QUIT:
                pygame.quit()
                running = False
            case _:
                continue

    # Update movement
    keys = pygame.key.get_pressed()
    if keys[currentPlayer.keys[0]]:
        position2D.y -= 1
    if keys[currentPlayer.keys[1]]:
        position2D.x -= 1
    if keys[currentPlayer.keys[2]]:
        position2D.y += 1
    if keys[currentPlayer.keys[3]]:
        position2D.x += 1

    # Fill the screen with a color
    screen.fill(WHITE)

    # Draw a blue rectangle
    pygame.draw.rect(screen, BLUE, (position2D.x, position2D.y, 200, 150))

    # Update the display
    pygame.display.flip()

# Quit Pygame
pygame.quit()
sys.exit()
