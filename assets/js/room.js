/**
 * Be sure to prefix import with ".js"
 * otherwise cannot serve it
 *
 **/
import {
  NEW_HAND,
  WAITING_FOR_MESSAGE,
  WAITING_FOR_PLAYERS,
  WEBSOCKET,
} from "./constants.js";
import { renderState } from "./render.js";
import { sendGetCurrentState } from "./messages.js";

let mode = WAITING_FOR_MESSAGE;

renderState(mode);

WEBSOCKET.onopen = sendGetCurrentState;
WEBSOCKET.onclose = () => {
  // websocket is closed.
  console.log("Connection is closed...");
};

WEBSOCKET.onerror = (err) => {
  console.error(`ws error ${err}`);
};

WEBSOCKET.onmessage = (evt) => {
  const roomMessage = JSON.parse(evt.data);
  console.log(roomMessage);

  if (roomMessage.msgType instanceof String) {
    console.log("roomMessage is a string type ", roomMessage.msgType);
  } else {
    if (roomMessage.msgType.waitingForPlayers) {
      mode = WAITING_FOR_PLAYERS;
    } else if (roomMessage.msgType.joined) {
      mode = WAITING_FOR_PLAYERS;
    } else if (roomMessage.msgType.newHand) {
      mode = NEW_HAND;
    } else if (roomMessage.msgType.receiveCards) {
      mode = EXCHANGE_CARDS;
    } else if (roomMessage.msgType.nextPlayerToReplaceCards) {
      mode = EXCHANGE_CARDS;
    } else if (roomMessage.msgType.nextPlayerToPlay) {
      mode = PLAYING_HAND;
    } else if (roomMessage.msgType.updateStackAndScore) {
      // todo set mode
    } else if (roomMessage.msgType.state) {
      // todo set mode
    }
  }
  renderState(mode);
};
