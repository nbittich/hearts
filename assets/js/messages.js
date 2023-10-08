import { WEBSOCKET as ws } from "./constants.js";
export function sendStringMessageType(msgType) {
  ws.send(
    JSON.stringify({
      msgType: msgType,
    }),
  );
}

export function sendGetCurrentState() {
  sendStringMessageType("getCurrentState");
}

export function sendReplaceCards(cards) {
  let obj = {
    replaceCards: cards,
  };
  sendStringMessageType(obj);
}

export function sendPlayCard(card) {
  let obj = {
    play: card,
  };
  sendStringMessageType(obj);
}

export function sendGetCards() {
  sendStringMessageType("getCards");
}

export function sendJoin() {
  sendStringMessageType("join");
}
export function sendJoinBot() {
  sendStringMessageType("joinBot");
}
