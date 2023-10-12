export const WAITING_FOR_MESSAGE = "WAITING_FOR_MESSAGE";
export const WAITING_FOR_PLAYERS = "WAITING_FOR_PLAYERS";
export const NEW_HAND = "NEW_HAND";
export const EXCHANGE_CARDS = "EXCHANGE_CARDS";
export const PLAYING_HAND = "PLAYING_HAND";
export const END = "END";

export const APP_DIV = document.getElementById("app");
export const WS_ENDPOINT = APP_DIV.dataset.wsEndpoint;
export const ROOM_ID = APP_DIV.dataset.roomId;
export const CURRENT_USER_ID = APP_DIV.dataset.userId;
export const WEBSOCKET = new WebSocket(`${WS_ENDPOINT}/${ROOM_ID}`);

// divs
export const STATE_DIV = document.querySelector(".gameState");
export const STACK_DIV = document.querySelector("#stack");
export const USERCARDS_DIV = document.querySelector("#playerBottomCards");
export const PLAYER_BOTTOM_DIV = document.querySelector("#playerBottom");
export const PLAYER_LEFT_DIV = document.querySelector("#playerLeft");
export const PLAYER_TOP_DIV = document.querySelector("#playerTop");
export const PLAYER_RIGHT_DIV = document.querySelector("#playerRight");
