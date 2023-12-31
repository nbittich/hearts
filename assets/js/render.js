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
      break;
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
    STACK_DIV.classList.remove("d-none");
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
  renderPlayers(seats);
}
export function renderPlayers(seats) {
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
export function renderNextPlayer(mode, currentPlayerId) {
  if (mode != PLAYING_HAND) {
    throw `invalid call to renderNextPlayer: ${mode}`;
  }
  let currentPlayerIdDiv = findPlayerDivById(currentPlayerId);
  if (!currentPlayerIdDiv) {
    throw `could not find playerDiv for currentPlayerId ${currentPlayerId}`;
  }
  renderPlayer(currentPlayerIdDiv, true, currentPlayerId);
}

export function renderScores(current_scores, player_scores) {
  const renderScoresBySpanClass = (player_scores, spanClass) => {
    for (const { player_id, score } of player_scores) {
      let playerIdDiv = findPlayerDivById(player_id);
      let scoreSpan = playerIdDiv.querySelector(spanClass);
      scoreSpan.innerText = score;
    }
  };
  renderScoresBySpanClass(current_scores, ".currentScore");
  renderScoresBySpanClass(player_scores, ".playerScore");
}

export function resetCurrentScores() {
  const ordered = getOrderedPlayerDivs(null);
  for (const { div } of ordered) {
    let scoreSpan = div.querySelector(".currentScore");
    scoreSpan.innerText = "0";
  }
}

export function renderNewHand(mode, playerIds, playerScores, currentPlayerId) {
  if (mode != NEW_HAND) {
    throw `invalid call to renderNewHand: ${mode}`;
  }
  renderState(mode);
  let orderedPlayerDivs = getOrderedPlayerDivs(playerIds);
  for (const { id, div } of orderedPlayerDivs) {
    renderPlayer(div, currentPlayerId === id, id);
  }
  renderScores(
    playerScores.map((p) => {
      return { player_id: p.player_id, score: 0 };
    }),
    playerScores,
  );
}
export function renderStack(mode, stack) {
  STACK_DIV.innerHTML = "";
  renderState(mode, (_) => {
    for (const card of stack) {
      renderCard(STACK_DIV, card, false);
    }
  });
}
export function renderReceivedCards(
  cards,
  onClick = (_cardElt, _clickedCard, _isSelected) => {
    throw "not implemented";
  },
) {
  USERCARDS_DIV.innerHTML = "";
  for (const card of cards) {
    renderCard(USERCARDS_DIV, card, true, onClick);
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
  let cardComponent = document.createElement(clickable && card ? "a" : "span");
  cardComponent.classList = "kard";
  if (!card) {
    cardComponent.textContent = "🂠";
    parentDiv.appendChild(cardComponent);
    return;
  }
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
      e.preventDefault();
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

export function renderCardSubmitButton(
  mode,
  renderCondition = false,
  onClick = (_) => { },
) {
  renderState(mode, (stateDiv) => {
    if (renderCondition) {
      let divButton = document.createElement("div");
      divButton.classList = "d-block";

      let button = document.createElement("button");
      button.href = "#";
      button.onclick = onClick;
      button.textContent = "Submit";
      divButton.appendChild(button);
      stateDiv.appendChild(divButton);
    }
  });
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

export function renderPlayer(playerDiv, currentPlayer = false, playerId) {
  if (playerId) {
    playerDiv.dataset.id = playerId;
  }
  let seatDiv = playerDiv.querySelector(".seat");
  let timerDiv = playerDiv.querySelector(".timer");
  seatDiv.classList = "seat";
  if (!playerId) {
    seatDiv.classList.add("emptySeat");
  } else {
    seatDiv.classList.add("filledSeat");
  }
  let playerNameP = playerDiv.querySelector(".playerName");
  playerNameP.innerText = playerId?.substring(0, 8) || "-";
  if (currentPlayer) {
    // reset previous current user
    let previousPlayerDiv = document.querySelector(
      `[data-current-player="true"]`,
    );
    if (previousPlayerDiv) {
      previousPlayerDiv.dataset.currentPlayer = false;
      let previousSeatDiv = previousPlayerDiv.querySelector(".seat");
      let previousTimerDiv = previousPlayerDiv.querySelector(".timer");
      previousSeatDiv.classList.remove("currentPlayer");
      previousTimerDiv.classList.add("d-none");
    }
    playerDiv.dataset.currentPlayer = true;
    seatDiv.classList.add("currentPlayer");
    timerDiv.classList.remove("d-none");
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
  if (!players?.length || players.every((p) => !p)) {
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
