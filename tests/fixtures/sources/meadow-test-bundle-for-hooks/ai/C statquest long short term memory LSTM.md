https://youtu.be/YCzL96nL7j0

Prerequisite is [[ref - statquest recurrent neural networks]]

### LSTM

At 4:10 Long short term memory neural networks use both of which two activation functions?:::sigmoid and tanh
<!--SR:!2022-12-20,23,290-->

At 6:10 what is the green line that runs all the way across the top of the unit called?::: The cell state
<!--SR:!2022-11-29,2,250-->

At 6:10 what does the cell state represent?::: The long term memory
<!--SR:!2022-12-11,13,250-->

At 6:20 what is it about the cell state that allows us to avoid the exploding and vanishing gradient?::: It does not have weights that it gets multiplied by.
<!--SR:!2022-12-09,11,250-->

At 6:30 what is the name for the thing that represents the short term memories?::: The hidden state
<!--SR:!2022-11-29,1,170-->

At 8:40 why is it important that we use the sigmoid activation function in the final step before multiplying it against the long-term memory?::: Because it results in a value of 0 to 1, so essentially a percentage that can be used to decide how much of the long-term memory you want to remember
<!--SR:!2022-12-13,15,270-->

At 9:10 what do you call the part of the long short term memory unit that determines what percentage of the long term memory to remember?::: The forget gate
<!--SR:!2022-12-06,9,230-->

At 12:20 what do you call the part of the long short term memory unit that detirmined how we should update the long term memory?::: The input gate
<!--SR:!2022-12-08,11,210-->

At 1420 what is the final output of an LSTM unit called?::: The output gate
<!--SR:!2022-12-18,21,270-->

At 1420 how many stages does an LSTM unit have?::: three
<!--SR:!2022-12-12,14,270-->

At 1640 what are the outputs for an LSTM unit?::: both a long term and a short term memory
<!--SR:!2022-12-14,16,270-->

#srs/ml 