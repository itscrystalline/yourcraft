#define SDL_MAIN_HANDLED
#include <stdio.h>
#include "SDL2/SDL.h"
#include <time.h>
#include <string.h>
// our function 
#include "main.h"
#include "struct.h"
#include "render.h"
#include "game.h"
#include "collision.h"

// MAIN FUNCTION
int main(int argc, char *argv[]){
  GameState mState;
  SDL_Window *window = NULL;
  SDL_Renderer *renderer = NULL;

  (void) argc;
  (void) argv;

  //The dimensions of the level
  const int LEVEL_WIDTH = 1920;
  const int LEVEL_HEIGHT = 1080;
  
  SDL_Init(SDL_INIT_VIDEO);

  window = SDL_CreateWindow("Game Window",
                            SDL_WINDOWPOS_UNDEFINED,
                            SDL_WINDOWPOS_UNDEFINED,
                            LEVEL_WIDTH,
                            LEVEL_HEIGHT,
                            0
                            );
  renderer = SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED  | SDL_RENDERER_PRESENTVSYNC );

  mState.renderer = renderer;
  loadGame(&mState);

  int done = 0;

  // main loop
  while(!done){

    done = processInputs(window, &mState);

    doRender(renderer, &mState);
    events(&mState);

  }

  SDL_DestroyWindow(window);
  SDL_DestroyRenderer(renderer);

  SDL_Quit();
  return 0;
}


