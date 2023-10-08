export const WAITING_FOR_MESSAGE = -1;
export const WAITING_FOR_PLAYERS = 0;
export const NEW_HAND = 1;
export const EXCHANGE_CARDS = 2;
export const PLAYING_HAND = 3;
export const END = 4;

export const APP_DIV = document.getElementById("app");
export const WS_ENDPOINT = APP_DIV.dataset.wsEndpoint;
export const ROOM_ID = APP_DIV.dataset.roomId;
export const CURRENT_USER_ID = APP_DIV.dataset.userId;
export const WEBSOCKET = new WebSocket(`${WS_ENDPOINT}/${ROOM_ID}`);
