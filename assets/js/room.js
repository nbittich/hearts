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
  renderNextPlayerToReplaceCards,
  renderCardSubmitButton,
} from "./render.js";
import {
  sendGetCards,
  sendGetCurrentState,
  sendReplaceCards,
} from "./messages.js";

let mode = WAITING_FOR_MESSAGE;
let playerIds = [];
let currentPlayerId = null;
let cardToPlay = null;
let cardsToExchange = null;
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
      currentPlayerId = current_player_id;
      renderNewHand(mode, playerIds, player_scores, currentPlayerId);
      if (playerIds.includes(CURRENT_USER_ID)) {
        sendGetCards();
      }
    } else if (roomMessage.msgType.receiveCards) {
      renderReceivedCards(roomMessage.msgType.receiveCards, handleCardClicked);
    } else if (roomMessage.msgType.nextPlayerToReplaceCards) {
      mode = EXCHANGE_CARDS;
      let { current_player_id } = roomMessage.msgType.nextPlayerToReplaceCards;
      currentPlayerId = current_player_id;

      renderNextPlayerToReplaceCards(mode, currentPlayerId);
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

function handleCardClicked(cardElt, clickedCard, isSelected) {
  //toggle
  switch (mode) {
    case EXCHANGE_CARDS:
    case NEW_HAND:
      cardsToExchange = cardsToExchange || [];

      if (isSelected) {
        cardsToExchange = cardsToExchange.filter(
          (c) => c.position_in_deck !== clickedCard.position_in_deck,
        );
        cardElt.classList.remove("kard-selected");
        cardElt.dataset.selected = false;
      } else if (cardsToExchange.length < 3) {
        cardsToExchange.push(clickedCard);
        cardElt.classList.add("kard-selected");
        cardElt.dataset.selected = true;
      }

      renderCardSubmitButton(
        mode,
        cardsToExchange.length === 3 && currentPlayerId === CURRENT_USER_ID,
        (evt) => {
          evt.preventDefault();
          sendReplaceCards(cardsToExchange);
          cardsToExchange = [];
          renderCardSubmitButton(mode); // remove submit button
          sendGetCards();
        },
      );

      break;
    case PLAYING_HAND:
      alert("playin hand");
      break;
    default:
      throw "handleCardClicked error: mode incorrect, " + mode;
  }
}
