import {
  WAITING_FOR_MESSAGE,
  END,
  EXCHANGE_CARDS,
  NEW_HAND,
  PLAYING_HAND,
  WAITING_FOR_PLAYERS,
  CURRENT_USER_ID,
  STATE_DIV,
  STACK_DIV,
  USERCARDS_DIV,
  PLAYER_BOTTOM_DIV,
  PLAYER_TOP_DIV,
  PLAYER_LEFT_DIV,
  PLAYER_RIGHT_DIV,
} from "./constants.js";
import { sendJoin, sendJoinBot } from "./messages.js";

export function renderState(mode, customizeStateDiv = (_stateDiv) => { }) {
  // reset state
  STATE_DIV.innerHTML = "";

  let message = null;

  switch (mode) {
    case WAITING_FOR_MESSAGE:
      message = "Waiting for message...";
      break;
    case WAITING_FOR_PLAYERS:
      message = "Waiting for players...";
      break;
    case NEW_HAND:
      message = "New hand...";
      break;
    case EXCHANGE_CARDS:
      message = "Exchange cards...";
      break;
    case PLAYING_HAND:
      break;
    case END:
      message = "End game."; // todo print the winner
    default:
      throw `unknown mode ${mode}`;
  }
  if (message) {
    // clear the stack div.
    STACK_DIV.innerHTML = "";
    STACK_DIV.classList.add("d-none");
    let span = document.createElement("span");
    span.innerText = message;
    STATE_DIV.appendChild(span);
    STATE_DIV.classList.remove("d-none");
  } else {
    STATE_DIV.classList.add("d-none");
  }
  customizeStateDiv(STATE_DIV);
}

export function renderWaitingForPlayers(mode, seats) {
  if (mode != WAITING_FOR_PLAYERS) {
    throw `invalid call to renderWaitingForPlayers: ${mode}`;
  }
  renderState(mode, (stateDiv) => {
    let divButton = document.createElement("div");
    divButton.classList = "d-block";

    if (!seats.includes(CURRENT_USER_ID)) {
      let joinButton = document.createElement("button");
      joinButton.onclick = sendJoin;
      joinButton.innerText = "Join";
      divButton.appendChild(joinButton);
    }
    let addBotButton = document.createElement("button");
    addBotButton.onclick = sendJoinBot;
    addBotButton.classList = "me-1";
    addBotButton.innerText = "Add bot";
    divButton.appendChild(addBotButton);
    stateDiv.appendChild(divButton);
  });
  let orderedPlayerDivs = getOrderedPlayerDivs(seats);
  for (const { id, div } of orderedPlayerDivs) {
    renderPlayer(div, false, id);
  }
}
export function renderNextPlayerToReplaceCards(mode, currentPlayerId) {
  if (mode != EXCHANGE_CARDS) {
    throw `invalid call to renderNextPlayerToReplaceCards: ${mode}`;
  }
  renderState(mode);
  let currentPlayerIdDiv = findPlayerDivById(currentPlayerId);
  if (!currentPlayerIdDiv) {
    throw `could not find playerDiv for currentPlayerId ${currentPlayerId}`;
  }
  renderPlayer(currentPlayerIdDiv, true, currentPlayerId);
}
export function renderNewHand(
  mode,
  playerIds,
  playersAndScores,
  currentPlayerId,
) {
  if (mode != NEW_HAND) {
    throw `invalid call to renderNewHand: ${mode}`;
  }
  renderState(mode);
  let orderedPlayerDivs = getOrderedPlayerDivs(playerIds);
  for (const { id, div } of orderedPlayerDivs) {
    let totalScore = playersAndScores.find((p) => p.player_id === id)?.score;
    renderPlayer(div, currentPlayerId === id, id, 0, totalScore);
  }
}

export function renderReceivedCards(cards) {
  USERCARDS_DIV.innerHTML = "";
  for (const card of cards) {
    renderCard(USERCARDS_DIV, card, true);
  }
}

export function renderCard(
  parentDiv,
  card,
  clickable = true,
  onClick = (cardElt, clickedCard, isSelected) => {
    console.log(
      `card ${clickedCard} clicked! is Selected = ${isSelected}, cardElt = ${cardElt}`,
    );
  },
) {
  let cardComponent = document.createElement(clickable ? "a" : "span");
  cardComponent.classList = "kard";
  switch (card.type_card) {
    case "CLUB":
    case "SPADE":
      cardComponent.classList.add("dark");
      break;
    case "HEART":
    case "DIAMOND":
      cardComponent.classList.add("crimson");
      break;
    default:
      throw `unknown type card ${card}`;
  }
  cardComponent.innerText = card.emoji;
  if (clickable) {
    cardComponent.onclick = (e) => {
      let cardElt = e.currentTarget;
      let clickedCard = JSON.parse(cardElt.dataset.card);
      let isSelected = cardElt.dataset.selected === "true";
      onClick(cardElt, clickedCard, isSelected);
    };
    cardComponent.href = "#";

    cardComponent.dataset.selected = false;
    cardComponent.dataset.card = JSON.stringify(card);
  }
  parentDiv.appendChild(cardComponent);
}

export function findPlayerDivById(playerId) {
  if (PLAYER_TOP_DIV.dataset.id === playerId) {
    return PLAYER_TOP_DIV;
  } else if (PLAYER_LEFT_DIV.dataset.id === playerId) {
    return PLAYER_LEFT_DIV;
  } else if (PLAYER_RIGHT_DIV.dataset.id === playerId) {
    return PLAYER_RIGHT_DIV;
  } else if (PLAYER_BOTTOM_DIV.dataset.id === playerId) {
    return PLAYER_BOTTOM_DIV;
  } else {
    console.log(`player ${playerId} not assigned to a div`);
  }
  return null;
}

export function renderPlayer(
  playerDiv,
  currentPlayer = false,
  playerId,
  currentScore = 0,
  totalScore = 0,
) {
  if (playerId) {
    playerDiv.dataset.id = playerId;
  }
  let seatDiv = playerDiv.querySelector(".seat");
  seatDiv.classList = "seat";
  if (!playerId) {
    seatDiv.classList.add("emptySeat");
  } else {
    seatDiv.classList.add("filledSeat");
  }
  let currentScoreSpan = playerDiv.querySelector(".score .currentScore");
  let totalScoreSpan = playerDiv.querySelector(".score .playerScore");
  let playerNameP = playerDiv.querySelector(".playerName");
  currentScoreSpan.innerText = currentScore;
  totalScoreSpan.innerText = totalScore;
  playerNameP.innerText = playerId?.substring(0, 8) || "-";
  if (currentPlayer) {
    // reset previous current user
    let previousPlayerDiv = document.querySelector(
      `[data-current-player="true"]`,
    );
    if (previousPlayerDiv) {
      previousPlayerDiv.dataset.currentPlayer = false;
      let previousSeatDiv = previousPlayerDiv.querySelector(".seat");
      previousSeatDiv.classList.remove("currentPlayer");
    }
    playerDiv.dataset.currentPlayer = true;
    seatDiv.classList.add("currentPlayer");
  }
}

export function getOrderedPlayerDivs(players) {
  // we always start with bottom player
  let ordered = [
    {
      id: null,
      div: PLAYER_BOTTOM_DIV,
    },
    {
      id: null,
      div: PLAYER_LEFT_DIV,
    },
    {
      id: null,
      div: PLAYER_TOP_DIV,
    },
    {
      id: null,
      div: PLAYER_RIGHT_DIV,
    },
  ];
  if (!players.length || players.every((p) => !p)) {
    return ordered;
  }
  if (players.includes(CURRENT_USER_ID)) {
    let idxCurrentUser = players.indexOf(CURRENT_USER_ID);
    let leftIdx = idxCurrentUser == 0 ? 3 : idxCurrentUser - 1;
    let rightIdx = idxCurrentUser < 3 ? idxCurrentUser + 1 : 0;
    let topIdx = idxCurrentUser < 2 ? idxCurrentUser + 2 : idxCurrentUser - 2;
    ordered[0].id = players[idxCurrentUser];
    ordered[1].id = players[leftIdx];
    ordered[2].id = players[topIdx];
    ordered[3].id = players[rightIdx];
  } else {
    for (let idx = 0; idx < players.length; idx++) {
      ordered[idx].id = players[idx];
    }
  }
  return ordered;
}
