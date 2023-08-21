const appDiv = document.getElementById("app");
const wsEndpoint = appDiv.dataset.wsEndpoint;
const roomId = appDiv.dataset.roomId;
const currentUserId = appDiv.dataset.userId;
const ws = new WebSocket(`${wsEndpoint}/${roomId}`);

const EXCHANGE_CARDS = 0;
const PLAYING_HAND = 1;

let isCurrentPlayer = false;

let mode = EXCHANGE_CARDS;

let cardsToExchange = [];

let cardToPlay = null;



ws.onopen = sendGetCurrentState;


ws.onmessage = function(evt) {
  const roomMessage = JSON.parse(evt.data);
  console.log(roomMessage);

  if (roomMessage.msgType instanceof String) {
    console.log("roomMessage is a string type ", roomMessage.msgType);
  } else {
    if (roomMessage.msgType.waitingForPlayers) {
      renderPlayers(roomMessage.msgType.waitingForPlayers);
    }
    else if (roomMessage.msgType.joined) {
      renderPlayerJoined(roomMessage.msgType.joined);
    }
    else if (roomMessage.msgType.newHand) {
      renderNewHand(roomMessage.msgType.newHand);
    }
    else if (roomMessage.msgType.receiveCards) {
      renderCards(roomMessage.msgType.receiveCards);
    }
    else if (roomMessage.msgType.nextPlayerToReplaceCards) {
      renderNextPlayer(roomMessage.msgType.nextPlayerToReplaceCards);
    }
    else if (roomMessage.msgType.nextPlayerToPlay) {
      let msg = roomMessage.msgType.nextPlayerToPlay;
      renderNextPlayer(msg);
      renderStack(msg);

    }
    else if (roomMessage.msgType.state) {
      let state = roomMessage.msgType.state;
      let playersDiv = appDiv.querySelector("#players");

      if (!playersDiv) {
        renderPlayers(state.player_scores.map(ps => ps.player_id));
        renderNextPlayer(state);
      }
      if (state.current_hand != state.hands) { // todo could be off by one
        renderCards(state.current_cards);

      }
      renderPlayersScore(state.player_scores);
      renderStack({ stack: state.current_stack });
    }

  }
};

ws.onclose = function() {
  // websocket is closed.
  console.log("Connection is closed...");
};


// helper send msg
function sendGetCurrentState() {
  sendStringMessageType("getCurrentState");
}

function sendReplaceCards(cards) {
  let obj = {
    replaceCards: cards
  };
  sendStringMessageType(obj);
}

function sendPlayCard(card) {
  let obj = {
    play: card
  };
  sendStringMessageType(obj);
}

function sendGetCards() {
  sendStringMessageType("getCards");
}

function sendJoin() {
  sendStringMessageType("join");
}
function sendJoinBot() {
  sendStringMessageType("joinBot");
}

function sendStringMessageType(msgType) {
  ws.send(JSON.stringify({
    msgType: msgType
  }));
}


// html render

const renderPlayer = (playersDiv, player) => {
  let p = document.createElement("p");
  p.textContent = `${currentUserId === player ? "You" : "Player " + player}`;
  p.dataset.userId = player;
  playersDiv.appendChild(p);
};
function renderPlayers(players) {
  // update screen with waiting for players

  let playersDiv = appDiv.querySelector("#players");

  if (playersDiv) {
    playersDiv.innerHTML = "";
  } else {
    playersDiv = document.createElement("div");
    playersDiv.id = "players";
    appDiv.appendChild(playersDiv);
  }

  let playerAlreadyJoined = players.includes(currentUserId);

  for (const player of players) {
    if (player)
      renderPlayer(playersDiv, player);
    else {
      let divJoinBlock = document.createElement("div");
      divJoinBlock.id = "join"
      divJoinBlock.className = "d-block";
      if (!playerAlreadyJoined) {
        let aJoin = document.createElement("a");
        aJoin.href = "#join";
        aJoin.textContent = "Join";
        aJoin.onclick = sendJoin;
        divJoinBlock.appendChild(aJoin);

      }

      let aJoinBot = document.createElement("a");
      aJoinBot.href = "#joinBot";
      aJoinBot.textContent = "Bot";
      aJoinBot.className = playerAlreadyJoined ? "" : "ms-1";
      aJoinBot.onclick = sendJoinBot;
      divJoinBlock.appendChild(aJoinBot);
      playersDiv.appendChild(divJoinBlock);
    }
  }

}
function renderPlayerJoined(player) {
  let playersDiv = appDiv.querySelector("#players");
  if (!playersDiv) {
    sendGetCurrentState();
  } else {
    const slot = [...playersDiv.children].find((child) => {
      return child.dataset.userId == null;
    });
    if (slot) {
      playersDiv.removeChild(slot);
      renderPlayer(playersDiv, player);
    }
  }

}

function renderNewHand(newHand) {
  renderPlayers(newHand.player_ids_in_order); // todo we probably want to have the score
  renderNextPlayer(newHand);
  sendGetCards();
  isCurrentPlayer = currentUserId === newHand.current_player_id;
  mode = EXCHANGE_CARDS;
  cardsToExchange = [];
  cardToPlay = null;
}

function renderCard(divCardsBlock, card) {
  if (!card) {
    return;
  }
  let aCard = document.createElement("a");
  aCard.href = "#playCard"; // todo either exchange cards or play
  aCard.textContent = card.emoji;
  aCard.onclick = handleCardPlayed;

  aCard.dataset.selected = false;
  aCard.dataset.card = JSON.stringify(card);
  switch (card.type_card) {
    case "CLUB":
    case "SPADE":
      aCard.classList = "me-1 dark card";
      break;
    case "DIAMOND":
    case "HEART":
      aCard.classList = "me-1 red card";
      break;
  }
  divCardsBlock.appendChild(aCard);
}
function renderCards(cards) {
  let divCardsBlock = appDiv.querySelector("#myCards");

  if (divCardsBlock) {
    divCardsBlock.innerHTML = "";
  } else {
    divCardsBlock = document.createElement("div");
    divCardsBlock.id = "myCards";
    appDiv.appendChild(divCardsBlock);
  }

  for (const card of cards) {
    renderCard(divCardsBlock, card);
  }
}


function renderNextPlayer({ current_player_id }) {
  isCurrentPlayer = current_player_id == currentUserId;
  let playersDiv = appDiv.querySelector("#players");
  let nextPlayerElt = [...playersDiv.childNodes]
    .map(elt => {
      elt.classList.remove('underline');
      return elt;
    })
    .find(p => p.dataset.userId === current_player_id);

  nextPlayerElt.classList = nextPlayerElt.classList + " underline";

}
function renderCardSubmitButton(renderCondition = false, onClick = (_) => { }) {
  let divCardsBlock = appDiv.querySelector("#myCards");

  let button = divCardsBlock.querySelector("#submitExchangeCards");

  if (button) {
    if (!renderCondition) {
      divCardsBlock.removeChild(button);
      return;
    } else {
      button.innerHTML = "";
    }
  } else {
    button = document.createElement('button');
  }

  if (renderCondition) {
    button.href = "#submitExchangeCards";
    button.id = "submitExchangeCards";
    button.onclick = onClick;
    button.textContent = "Submit";
    divCardsBlock.appendChild(button);

  }
}

function renderPlayersScore(player_scores) {
  let playersDiv = appDiv.querySelector("#players");
  let playersElt = [...playersDiv.childNodes];
  for (const playerElt of playersElt) {
    let userId = playerElt.dataset.userId;
    let scoreElt = playerElt.querySelector('.score');

    if (scoreElt) {
      scoreElt.innerHTML = "";
    } else {
      scoreElt = document.createElement("span");
      playerElt.appendChild(scoreElt);
    }
    scoreElt.classList = "score";
    let player_score = player_scores.find(ps => ps.player_id == userId);
    scoreElt.innerText = `(${player_score.score})`;

  }
}
function renderStack({ stack }) {
  let stackDiv = appDiv.querySelector("#stackDiv");
  if (stackDiv) {
    stackDiv.innerHTML = "";
  } else {
    stackDiv = document.createElement("div");
    separator = document.createElement("hr");
    stackDiv.id = "stackDiv";
    stackDiv.classList = "d-block";
    appDiv.append(separator);
    appDiv.append(stackDiv);
  }
  for (const card of stack) {
    if (card) {
      renderCard(stackDiv, card);
    }
  }



}

// HANDLER
//
function handleCardPlayed(e) {
  e.preventDefault();

  if (isCurrentPlayer) {
    let cardElt = e.currentTarget;
    let clickedCard = JSON.parse(cardElt.dataset.card);
    switch (mode) {
      case EXCHANGE_CARDS:
        cardsToExchange = cardsToExchange || [];
        if (cardElt.dataset.selected === "true") {
          cardsToExchange = cardsToExchange.filter(c => c.position_in_deck !== clickedCard.position_in_deck);
          cardElt.classList.remove('card-selected');
          cardElt.dataset.selected = false;
        }
        else if (cardsToExchange.length < 3) {
          cardsToExchange.push(clickedCard);
          cardElt.classList.add('card-selected');
          cardElt.dataset.selected = true;
        }
        renderCardSubmitButton(cardsToExchange.length === 3, (evt) => {
          evt.preventDefault();
          sendReplaceCards(cardsToExchange);
          cardsToExchange = [];
          renderCardSubmitButton(); // remove submit button
          sendGetCurrentState();

          mode = PLAYING_HAND;

        });

        break;
      case PLAYING_HAND:
        if (cardElt.dataset.selected === "true") {
          cardToPlay = null;
          cardElt.classList.remove('card-selected');
          cardElt.dataset.selected = false;
        } else {
          cardToPlay = clickedCard;
          cardElt.classList.add('card-selected');
          cardElt.dataset.selected = true;
        }
        renderCardSubmitButton(cardToPlay, (evt) => {
          evt.preventDefault();
          sendPlayCard(cardToPlay);
          cardToPlay = null;
          renderCardSubmitButton(); // remove submit button
          sendGetCurrentState();
        });
    }

  }
}
