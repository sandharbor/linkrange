https://youtu.be/AsNTP8Kwu80

In the recurrent neural networks video, what was different about the inputs as compared to other neural networks videos?::: The inputs have different amounts of data. They were timeseries and the historical data they had was for different amounts of time because the companies had existed for a different amount of time.
<!--SR:!2022-12-02,9,250-->

At seven minutes in a recurrent neural network, what does the feedback loop feed back into?::: a summation
<!--SR:!2024-04-29,27,270-->

At 11:00 [[diff - vanishing vs. exploding gradient]]

At maybe 12, describe the weights and biases associated with the unrolled recurrent neural network.:: They are the same for every iteration, so you only need to train a single set of weights and biases
<!--SR:!2022-12-01,11,270-->

At 14 why do recurrent neural networks have [[diff - vanishing vs. exploding gradient]]?::: because each iteration's output has a weight associated with it, and if you have some thing like 50 iterations, if the weight is greater than one it gets huge, and if it's less than one it gets close to zero.
<!--SR:!2022-12-10,19,290-->

at 14 if we set some hyperparameter wrong, then during [[gradient descent]] the [[exploding gradient problem]] can show up.  How does that happen?::: if you set the learning rate too high, then instead of taking small steps, when the gradient is very large, we will take large steps, and instead of trending towards the optimal value ( [[convergence]] ) we will just bounce all over the place
<!--SR:!2024-05-23,51,290-->

At 15 why is a [[vanishing gradient problem]] a problem when doing gradient descent?::: Because gradient descent makes such a small change each time that we exceed the maximum number of steps we can take before we find the optimal value.
<!--SR:!2024-05-20,48,290-->


#flashcards 