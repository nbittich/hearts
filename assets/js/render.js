import {
  WAITING_FOR_MESSAGE,
  END,
  EXCHANGE_CARDS,
  NEW_HAND,
  PLAYING_HAND,
  WAITING_FOR_PLAYERS,
} from "./constants.js";

export function renderState(mode) {
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
}
