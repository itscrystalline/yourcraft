#include <stdio.h>
#include "SDL2/SDL.h"
#include <time.h>
#include <string.h>
// our function
#include "collision.h"
#include "struct.h"
#include "render.h"

int playerCollision (Man *man, float obj_x[2], float obj_y[2]){
  // edge bottom
  int bottomEnv = (man->cy[0] < obj_y[1]) && (obj_y[1] < man->cy[1]);
  int bottomPla = (obj_y[0] < man->cy[1]) && (man->cy[1] < obj_y[1]);
  int bottom = bottomEnv || bottomPla;
  // edge top
  int topEnv = (man->cy[0] < obj_y[0]) && (obj_y[0] < man->cy[1]);
  int topPla = (obj_y[0] < man->cy[0]) && (man->cy[0] < obj_y[1]);
  int top = topEnv || topPla;
  // edge left
  int rightEnv = (man->cx[0] < obj_x[1]) && (obj_x[1] < man->cx[1]);
  int rightPla = (obj_x[0] < man->cx[1]) && (man->cx[1] < obj_x[1]);
  int right = rightEnv || rightPla;
  // edge right
  int leftEnv= (man->cx[0] < obj_x[0]) && (obj_x[0] < man->cx[1]);
  int leftPla = (obj_x[0] < man->cx[0]) && (man->cx[0] < obj_x[1]);
  int left = leftEnv || leftPla;

  // check collision value
  float overlap_x, overlap_y = 0 ;
  int direction[2] = {0,0};

  // X axis
  if ( bottom || top ) {
  // X axis Right
    if ( man->cx[1]>obj_x[0] && man->cx[1]<obj_x[1]){
      overlap_x = obj_x[0] - man->cx[1];
      direction[0] = 1;
    }
  // X axis Left
    else if ( man->cx[0]<obj_x[1] && man->cx[0]>obj_x[0]){
      overlap_x = man->cx[0] - obj_x[1];
      direction[0] = 2;
    }
    else {
      direction[0] = 0;
      overlap_x = 0;
    }
  }
  else {
    direction[0] = 0;
    overlap_x = 0;
  }
  // Y axis
  if ( left || right ) {
  // Y axis Bottom
    if ( man->cy[1]>obj_y[0] && man->cy[1]<obj_y[1]){
      overlap_y = obj_y[0] - man->cy[1];
      direction[1] = 1;
    }
  // Y axis Top
    else if ( man->cy[0]<obj_y[1] && man->cy[0]>obj_y[0]){
      overlap_y = man->cy[0] - obj_y[1];
      direction[1] = 2;
    }
    else {
      direction[1] = 0;
      overlap_y = 0;
    }
  }
  else {
    direction[1] = 0;
    overlap_y = 0;
  }

  // apply collision to the most overlaped
  if (overlap_x == 0 && overlap_y == 0 ){
    // printf("no collision\n");
    return 0;
  }
  else if (overlap_x > overlap_y || overlap_y == 0){
   if ( direction[0] == 1){
    // right
      man->x = obj_x[0]-man->size_x;
      if (man->vx>0){man->vx=0;}
      return 1;
    }
    // left
    else if ( direction[0] == 2){
      man->x = obj_x[1];
      if (man->vx<0){man->vx=0;}
      return 2;
    }
  }
  else if (overlap_y > overlap_x || overlap_x == 0){
    // down
    if ( direction[1] == 1){
      man->y = obj_y[0]-man->size_y;
      // player movement
      if (man->vy>0){man->vy=0;}
      // man->onFloor = 1;
      // man->inAir = 0;
      return 3;
    }
    // up
    else if ( direction[1] == 2){
      man->y = obj_y[1];
      if (man->vy<0){man->vy=0;}
      return 4;
    }
  }
  
  return 0;
}

