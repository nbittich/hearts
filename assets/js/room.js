/**
 * Be sure to prefix import with ".js"
 * otherwise cannot serve it
 *
 **/
import {
  NEW_HAND,
  WAITING_FOR_MESSAGE,
  WAITING_FOR_PLAYERS,
  EXCHANGE_CARDS,
  PLAYING_HAND,
  WEBSOCKET,
  CURRENT_USER_ID,
} from "./constants.js";
import {
  renderState,
  renderWaitingForPlayers,
  renderNewHand,
  renderReceivedCards,
} from "./render.js";
import { sendGetCards, sendGetCurrentState } from "./messages.js";

let mode = WAITING_FOR_MESSAGE;
let playerIds = [];

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
      playerIds = roomMessage.msgType.waitingForPlayers;
      renderWaitingForPlayers(mode, playerIds);
    } else if (roomMessage.msgType.joined) {
      if (mode != WAITING_FOR_PLAYERS) {
        throw `joined event and invalid mode ${mode}`;
      }
      let emptySeat = playerIds.indexOf(null);
      playerIds[emptySeat] = roomMessage.msgType.joined;
      renderWaitingForPlayers(mode, playerIds);
    } else if (roomMessage.msgType.newHand) {
      mode = NEW_HAND;
      let { player_ids_in_order, player_scores, current_player_id } =
        roomMessage.msgType.newHand;
      playerIds = player_ids_in_order;
      renderNewHand(mode, playerIds, player_scores, current_player_id);
      if (playerIds.includes(CURRENT_USER_ID)) {
        sendGetCards();
      }
    } else if (roomMessage.msgType.receiveCards) {
      renderReceivedCards(roomMessage.msgType.receiveCards);
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
  //renderState(mode);
};
