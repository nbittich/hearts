import {
  WAITING_FOR_MESSAGE,
  END,
  EXCHANGE_CARDS,
  NEW_HAND,
  PLAYING_HAND,
  WAITING_FOR_PLAYERS,
  CURRENT_USER_ID,
} from "./constants.js";
import { sendJoin, sendJoinBot } from "./messages.js";

export function renderState(mode, customizeStateDiv = (stateDiv) => { }) {
  // reset state
  const stateDiv = document.querySelector(".gameState");
  stateDiv.innerHTML = "";

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
    const stackDiv = document.querySelector("#stack");
    stackDiv.innerHTML = "";
    stackDiv.classList.add("d-none");
    let span = document.createElement("span");
    span.innerText = message;
    stateDiv.appendChild(span);
    stateDiv.classList.remove("d-none");
  } else {
    stateDiv.classList.add("d-none");
  }
  customizeStateDiv(stateDiv);
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
export function renderPlayer(
  playerDiv,
  currentPlayer = false,
  playerId,
  currentScore = 0,
  totalScore = 0,
) {
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
    seatDiv.classList.add("currentPlayer");
  }
}

export function getOrderedPlayerDivs(players) {
  // we always start with bottom player as current user
  let ordered = [
    {
      id: null,
      div: document.querySelector("#playerBottom"),
    },
    {
      id: null,
      div: document.querySelector("#playerLeft"),
    },

    {
      id: null,
      div: document.querySelector("#playerRight"),
    },

    {
      id: null,
      div: document.querySelector("#playerTop"),
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
    ordered[2].id = players[rightIdx];
    ordered[3].id = players[topIdx];
  } else {
    for (let idx = 0; idx < players.length; idx++) {
      ordered[idx].id = players[idx];
    }
  }
  return ordered;
}
